(function() {
  'use strict';
  
  const ARCAFERRY_BUILD = 'share-page@1.0.1+inject-chat';
  
  const API_BASE = 'https://quack.im';
  
  const AVATAR_BASE = `${API_BASE}/upload_char_avatar/`;
  
  const PLACEHOLDER_PATTERNS = [
    /\{\{hidden:([^}]+)\}\}/g,
    /\{\{secret:([^}]+)\}\}/g,
    /\[HIDDEN:([^\]]+)\]/g,
    /<hidden>([^<]+)<\/hidden>/gi
  ];
  
  function extractShareId() {
    const url = window.location.pathname;
    
    const shareMatch = url.match(/\/discovery\/share\/([^/?]+)/);
    if (shareMatch) return shareMatch[1];
    
    return null;
  }
  
  function detectPlaceholders(text) {
    if (!text) return [];
    
    const keys = [];
    for (const pattern of PLACEHOLDER_PATTERNS) {
      pattern.lastIndex = 0;
      let match = pattern.exec(text);
      while (match !== null) {
        keys.push(match[1].trim());
        match = pattern.exec(text);
      }
    }
    return [...new Set(keys)];
  }
  
  function getHiddenAttrKeys(customAttrs) {
    if (!Array.isArray(customAttrs)) return [];
    
    return customAttrs
      .filter((attr) => attr && attr.isVisible === false)
      .map((attr) => attr.label || attr.name)
      .filter(Boolean);
  }
  
  function buildAvatarUrl(picture) {
    if (!picture) return '';
    if (picture.startsWith('http')) return picture;
    return AVATAR_BASE + picture;
  }
  
  async function fetchCharacterInfo(shareId) {
    const url = `${API_BASE}/api/v1/studioCard/info?isguest=1&sid=${shareId}`;
    console.log('[Arcaferry] Fetching basic info from:', url);
    
    const response = await fetch(url, {
      credentials: 'include',
      headers: { 'Accept': 'application/json' }
    });
    
    if (!response.ok) {
      throw new Error(`API request failed: ${response.status}`);
    }
    
    const data = await response.json();
    
    if (data.code !== 0) {
      throw new Error(data.msg || 'API error');
    }
    
    return data.data;
  }
  
  function extractBasicInfo(apiData, shareId) {
    const allAttrs = [];

    const firstChar = apiData?.charList?.[0];
    const takeAttrs = (list) => {
      if (!Array.isArray(list)) return;
      allAttrs.push(...list);
    };

    takeAttrs(apiData?.customAttrs);
    takeAttrs(firstChar?.attrs);
    takeAttrs(firstChar?.adviseAttrs);

    const customStart = allAttrs.length;
    const customList = Array.isArray(firstChar?.customAttrs) ? firstChar.customAttrs : [];
    takeAttrs(customList);

    const hiddenIndices = [];
    for (let i = 0; i < customList.length; i += 1) {
      const attr = customList[i];
      const label = attr?.label || attr?.name || '';
      const value = attr?.value;
      if (!label) continue;
      if (attr?.isVisible !== false) continue;
      if (value) continue;
      hiddenIndices.push({ attrIndex: i, label, mergedIndex: customStart + i });
    }

    const hiddenKeys = hiddenIndices.map((x) => x.label);

    return {
      basicInfo: {
        // Match Pro (Rust) precedence: prefer charList[0] fields when available.
        name: firstChar?.name || apiData.name || '',
        description: apiData.description || apiData.intro || firstChar?.intro || '',
        personality: apiData.personality || '',
        scenario: apiData.scenario || '',
        first_mes: apiData.first_mes || apiData.firstMes || '',
        mes_example: apiData.mes_example || apiData.mesExample || '',
        // Match Pro (Rust) precedence: prefer firstChar.prompt over info.system_prompt.
        system_prompt: firstChar?.prompt || apiData.system_prompt || apiData.systemPrompt || '',
        post_history_instructions: apiData.post_history_instructions || apiData.postHistoryInstructions || '',
        // Keep both creator and author_name so mapping can mirror Rust fallbacks.
        creator: apiData.creator || apiData.author || '',
        author_name: apiData.author || apiData.authorName || '',
        creator_notes: apiData.creator_notes || apiData.creatorNotes || apiData.intro || firstChar?.intro || '',
        customAttrs: allAttrs,
        // Upstream may put greetings under prologue.greetings; keep prologue for mapping.
        greeting: apiData.greeting || [],
        prologue: apiData.prologue || null,
        intro: apiData.intro || firstChar?.intro || '',
        avatarUrl: buildAvatarUrl(firstChar?.picture || apiData.picture),
        characterbooks: apiData.characterbooks || [],
        shareId,
        tags: apiData.tags || []
      },
      hiddenKeys,
      hiddenIndices
    };
  }
  
  async function extractAndSend() {
    const shareId = extractShareId();
    if (!shareId) {
      console.log('[Arcaferry] Not a share page');
      return;
    }
    
    console.log('[Arcaferry] Build:', ARCAFERRY_BUILD);
  console.log('[Arcaferry] Detected share page:', shareId);
    
    try {
      const apiData = await fetchCharacterInfo(shareId);
      const { basicInfo, hiddenKeys, hiddenIndices } = extractBasicInfo(apiData, shareId);
      
      console.log('[Arcaferry] Extracted:', basicInfo.name);
      console.log('[Arcaferry] Hidden keys:', hiddenKeys);
      
      try {
        window.__ARCAFERRY_SHARE_STATE__ = { basicInfo, hiddenKeys, hiddenIndices, shareId };
      } catch {
      }

      chrome.runtime.sendMessage({
        action: 'saveBasicInfo',
        data: {
          basicInfo: { ...basicInfo, __source: 'share' },
          hiddenKeys,
          hiddenIndices,
          shareId
        }
      });
      
    } catch (err) {
      console.error('[Arcaferry] Extraction failed:', err);
    }
  }
  
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.action === 'extractBasicInfo') {
      extractAndSend().then(() => {
        sendResponse({ success: true });
      }).catch(err => {
        sendResponse({ success: false, error: err.message });
      });
      return true;
    }
  });
  
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
      // Manual mode: do not auto-run extractAndSend
      console.log('[Arcaferry] Share page loaded (manual mode)');
    });
  } else {
    // Manual mode: do not auto-run extractAndSend
    console.log('[Arcaferry] Share page loaded (manual mode)');
  }
  
})();
