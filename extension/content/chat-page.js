(function() {
  'use strict';
  
  const ARCAFERRY_BUILD = 'chat-page@1.0.2+studio-api';
  
  const API_BASE = 'https://quack.im';

  const AVATAR_BASE = `${API_BASE}/upload_char_avatar/`;

  function buildAvatarUrl(picture) {
    if (!picture || typeof picture !== 'string') return '';
    if (picture.startsWith('http')) return picture;
    return `${AVATAR_BASE}${picture}`;
  }
  
  function valueToString(v) {
    if (v === null || v === undefined) return '';
    if (typeof v === 'string') return v;
    if (typeof v === 'number') return String(v);
    return String(v);
  }
  
  function getCookieMap() {
    const map = {};
    const raw = document.cookie || '';
    raw.split(';').forEach(pair => {
      const index = pair.indexOf('=');
      if (index === -1) return;
      const key = pair.slice(0, index).trim();
      const value = pair.slice(index + 1).trim();
      if (key) map[key.toLowerCase()] = decodeURIComponent(value);
    });
    return map;
  }
  
  function getCsrfTokenFromCookies() {
    const cookies = getCookieMap();
    const keys = ['xsrf-token', 'x-csrf-token', 'csrf-token', 'csrftoken', 'csrf'];
    for (const key of keys) {
      if (cookies[key]) return cookies[key];
    }
    return '';
  }
  
  function extractTokenFromValue(value) {
    if (!value || typeof value !== 'string') return '';
    const trimmed = value.trim();
    if (!trimmed) return '';
    if (trimmed.startsWith('Bearer ')) return trimmed.slice(7);
    const jwtPattern = /^[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$/;
    if (jwtPattern.test(trimmed)) return trimmed;
    return trimmed.length >= 20 ? trimmed : '';
  }
  
  function findAuthTokenFromStorage(store) {
    try {
      for (let i = 0; i < store.length; i += 1) {
        const key = store.key(i);
        if (!key) continue;
        const value = store.getItem(key);
        if (!value) continue;
        if (/token|auth/i.test(key)) {
          const token = extractTokenFromValue(value);
          if (token) return token;
        }
        const directToken = extractTokenFromValue(value);
        if (directToken) return directToken;
        try {
          const parsed = JSON.parse(value);
          if (parsed && typeof parsed === 'object') {
            const candidates = [
              parsed.token,
              parsed.accessToken,
              parsed.access_token,
              parsed.authToken,
              parsed.authorization,
              parsed.idToken,
              parsed.data?.token,
              parsed.data?.accessToken,
              parsed.data?.access_token
            ];
            for (const candidate of candidates) {
              const token = extractTokenFromValue(candidate);
              if (token) return token;
            }
          }
        } catch {
        }
      }
    } catch {
    }
    return '';
  }
  
  function getAuthTokenFromStorage() {
    return (
      findAuthTokenFromStorage(window.localStorage) ||
      findAuthTokenFromStorage(window.sessionStorage)
    );
  }
  
  function buildRequestHeaders(includeAuth = false) {
    const headers = {
      'Accept': 'application/json, text/plain, */*',
      'X-Requested-With': 'XMLHttpRequest'
    };
    const csrfToken = getCsrfTokenFromCookies();
    if (csrfToken) {
      headers['X-CSRF-Token'] = csrfToken;
      headers['X-XSRF-TOKEN'] = csrfToken;
    }
    if (includeAuth) {
      const token = capturedAuthToken || getAuthTokenFromStorage();
      if (token) headers['Authorization'] = `Bearer ${token}`;
    }
    return headers;
  }

  function extractPersonaNameFromChatInfo(chatInfo) {
    const tryExtract = (obj, prefix) => {
      try {
        const direct = obj?.personaName || obj?.persona_name;
        if (typeof direct === 'string' && direct.trim()) {
          return { name: direct.trim(), source: `${prefix}.personaName` };
        }

        const personas = obj?.personas || obj?.personaList || obj?.persona_list;
        if (Array.isArray(personas) && personas.length) {
          const active = personas.find((p) => p?.isActive) || personas.find((p) => p?.active) || personas[0];
          const name = active?.name || active?.personaName || active?.persona_name;
          if (typeof name === 'string' && name.trim()) {
            return { name: name.trim(), source: `${prefix}.personas` };
          }
        }

        const p = obj?.persona || obj?.activePersona || obj?.active_persona;
        const name2 = p?.name || p?.personaName || p?.persona_name;
        if (typeof name2 === 'string' && name2.trim()) {
          return { name: name2.trim(), source: `${prefix}.activePersona` };
        }
      } catch {
      }
      return null;
    };

    // Some payloads nest persona under chatInfo.chatInfo (observed in info-by-chat-index).
    const r1 = tryExtract(chatInfo, 'chatInfo');
    if (r1) return r1;
    const r2 = tryExtract(chatInfo?.chatInfo, 'chatInfo.chatInfo');
    if (r2) return r2;

    return { name: '', source: 'none' };
  }
  
  function findIndexFromUrl() {
    try {
      const url = new URL(window.location.href);
      const pathMatch = url.pathname.match(/\/dream\/([^/?]+)/);
      if (pathMatch) return pathMatch[1];
      return url.searchParams.get('index') || '';
    } catch {
      return '';
    }
  }
  
  let capturedAuthToken = '';
  let capturedChatInfo = null;

  // Strong manual mode: extraction happens ONLY after user clicks Start.
  let extractionEnabled = false;
  
  const capturedState = { cid: '', index: '' };
  
  function setCapturedState(data) {
    if (!data) return;
    const cidValue = data.cid || data.sid || data.chatId;
    if (cidValue) capturedState.cid = valueToString(cidValue);
    if (data.index) capturedState.index = valueToString(data.index);
    if (capturedState.cid || capturedState.index) {
      window.__ARCAFERRY_CAPTURED__ = {
        cid: capturedState.cid,
        index: capturedState.index
      };
    }
  }
  
  window.addEventListener('message', (event) => {
    if (event.source !== window) return;
    const payload = event.data;
    if (payload?.type === 'ARCAFERRY_AUTH') {
      const token = payload?.payload?.token;
      const extracted = extractTokenFromValue(token);
      if (extracted && extracted !== capturedAuthToken) {
        capturedAuthToken = extracted;
      }
      return;
    }
    if (payload?.type === 'ARCAFERRY_CAPTURED') {
      setCapturedState(payload.payload);
      if (payload.chatInfo) {
        capturedChatInfo = payload.chatInfo;
      }
      console.log('[Arcaferry] Captured from page:', capturedState);
    }

    if (payload?.type === 'ARCAFERRY_WORLDBOOK') {
      const wb = payload.payload;
      if (wb?.worldbook?.length) {
        try {
          window.__ARCAFERRY_CAPTURED_WORLDBOOK__ = wb;
        } catch {
        }
        if (isPlaceholderEntries(wb.worldbook)) {
          console.log('[Arcaferry] Captured placeholder worldbook from page; ignoring');
          return;
        }
        console.log('[Arcaferry] Captured worldbook from page:', wb.worldbook.length);
      }
    }
  });
  
  let hasSharedStudioData = false;
  
  async function handleCapturedChatInfo(chatInfo) {
    if (!extractionEnabled) return;
    if (hasSharedStudioData) {
      console.log('[Arcaferry] Already have share data, skipping');
      return;
    }
    
    const originSid = chatInfo?.originSid;
    
    const studioData = await fetchStudioCardInfo(originSid);
    
    // Persona name is optional; used only for {{user}} normalization.
    const persona = extractPersonaNameFromChatInfo(chatInfo);
    const personaName = persona?.name || '';
    if (personaName) {
      console.log('[Arcaferry] Detected personaName:', personaName, '(source:', persona?.source || 'unknown', ')');
    } else {
      console.log('[Arcaferry] Detected personaName: (empty)');
    }

    if (studioData) {
      hasSharedStudioData = true;
      const { basicInfo, hiddenKeys, hiddenIndices } = extractBasicInfoFromStudioCard(studioData, originSid);

      if (personaName) {
        basicInfo.personaName = personaName;
      }

      console.log('[Arcaferry] Saved basicInfo.personaName:', basicInfo.personaName || '(empty)');
      
      console.log('[Arcaferry] Got studio card data:', basicInfo.customAttrs.length, 'attrs');
      
      chrome.runtime.sendMessage({
        action: 'saveBasicInfo',
        data: {
          basicInfo,
          hiddenKeys,
          hiddenIndices,
          shareId: originSid
        }
      }).catch(() => {});
    } else {
      const basicInfo = buildBasicInfoFromChatInfo(chatInfo);

      if (personaName) {
        basicInfo.personaName = personaName;
      }

      console.log('[Arcaferry] Saved basicInfo.personaName:', basicInfo.personaName || '(empty)');

      const { hiddenKeys, hiddenIndices } = extractHiddenFromChatInfo(chatInfo);

      chrome.runtime.sendMessage({
        action: 'saveBasicInfo',
        data: {
          basicInfo,
          hiddenKeys,
          hiddenIndices,
          shareId: null
        }
      }).catch(() => {});
    }
  }
  
  function findCidAndIndexFromPage() {
    const result = { cid: '', index: '' };
    
    const nextDataScript = document.querySelector('script#__NEXT_DATA__');
    if (nextDataScript) {
      try {
        const data = JSON.parse(nextDataScript.textContent);
        const props = data?.props?.pageProps;
        
        if (props?.chatInfo) {
          result.cid = valueToString(props.chatInfo.cid);
          result.index = valueToString(props.chatInfo.index);
        }
        if (props?.cid) result.cid = valueToString(props.cid);
        if (props?.index) result.index = valueToString(props.index);
        
        console.log('[Arcaferry] From __NEXT_DATA__:', result);
        if (result.cid && result.index) return result;
      } catch (e) {
        console.warn('[Arcaferry] Failed to parse __NEXT_DATA__:', e);
      }
    }
    
    const scripts = document.querySelectorAll('script:not([src])');
    for (const script of scripts) {
      const content = script.textContent || '';
      
      if (!result.cid) {
        const cidMatch = content.match(/["']cid["']\s*:\s*["']([^"']+)["']/);
        if (cidMatch) result.cid = cidMatch[1];
      }
      
      if (!result.index) {
        const indexMatch = content.match(/["']index["']\s*:\s*["']([^"']+)["']/);
        if (indexMatch) result.index = indexMatch[1];
      }
      
      if (result.cid && result.index) break;
    }
    
    console.log('[Arcaferry] From inline scripts:', result);
    return result;
  }
  
  function findFromNetworkRequests() {
    const result = {
      cid: capturedState.cid,
      index: capturedState.index
    };
    
    if (window.__ARCAFERRY_CAPTURED__) {
      result.cid = result.cid || window.__ARCAFERRY_CAPTURED__.cid || '';
      result.index = result.index || window.__ARCAFERRY_CAPTURED__.index || '';
    }
    
    return result;
  }
  
  function waitForCaptured({ needCid = true, needIndex = true, timeoutMs = 3000 } = {}) {
    return new Promise((resolve) => {
      const start = Date.now();
      const check = () => {
        const result = findFromNetworkRequests();
        const hasCid = !needCid || !!result.cid;
        const hasIndex = !needIndex || !!result.index;
        if (hasCid && hasIndex) return resolve(result);
        if (Date.now() - start >= timeoutMs) return resolve(result);
        setTimeout(check, 200);
      };
      check();
    });
  }
  
  function getWorldbookFromPageCapture() {
    try {
      const wb = window.__ARCAFERRY_CAPTURED_WORLDBOOK__;
      if (wb?.worldbook && Array.isArray(wb.worldbook)) {
        return wb.worldbook;
      }
    } catch {
    }
    return [];
  }

  async function waitForWorldbookCapture(timeoutMs = 1500) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      const captured = getWorldbookFromPageCapture();
      if (captured.length) return captured;
      await new Promise((r) => setTimeout(r, 100));
    }
    return [];
  }

  function isPlaceholderEntry(entry) {
    if (!entry) return true;
    const content = (entry.content || '').trim();
    if (!content) return true;
    if (content === '_' || content === '-') return true;
    if (content.length <= 1) return true;
    return false;
  }

  // Match Pro behavior: treat whole lorebook as placeholder if ANY entry is placeholder.
  function isPlaceholderEntries(entries) {
    if (!Array.isArray(entries) || entries.length === 0) return false;
    return entries.some((e) => isPlaceholderEntry(e));
  }

  async function getTabIdViaBackground() {
    try {
      const resp = await chrome.runtime.sendMessage({ action: 'getTabId' });
      return resp?.tabId || null;
    } catch {
      return null;
    }
  }

  async function fetchWorldbookViaBackground(index, cid) {
    const tabId = await getTabIdViaBackground();
    if (!tabId) return { success: false, entries: [], error: 'Missing tabId' };

    try {
      const resp = await chrome.runtime.sendMessage({
        action: 'fetchWorldbook',
        data: { tabId, index, cid, apiBase: API_BASE, authToken: capturedAuthToken }
      });
      if (!resp?.success) return { success: false, entries: [], error: resp?.error || 'Worldbook API failed' };

      const entries = Array.isArray(resp.entries) ? resp.entries : [];
      if (entries.length > 0 && isPlaceholderEntries(entries)) {
        // Treat placeholders as empty success (matches Pro: placeholders must not persist).
        return { success: true, entries: [] };
      }
      return { success: true, entries };
    } catch {
      return { success: false, entries: [], error: 'Worldbook API failed' };
    }
  }

  async function fetchWorldbook(index, cid) {
    const captured = getWorldbookFromPageCapture();
    if (captured.length && !isPlaceholderEntries(captured)) return captured;

    await waitForCaptured({ needCid: false, needIndex: false, timeoutMs: 3000 });
    const afterWait = getWorldbookFromPageCapture();
    if (afterWait.length && !isPlaceholderEntries(afterWait)) return afterWait;

    const res = await fetchWorldbookViaBackground(index, cid);
    if (!res.success) {
      throw new Error(res.error || 'Worldbook API failed');
    }
    return res.entries || [];
  }
  
  async function fetchChatInfo(index) {
    if (!index) return null;
    const url = `${API_BASE}/api/v1/user/character/info-by-chat-index?index=${index}`;
    console.log('[Arcaferry] Fetching chat info from:', url);
    
    const response = await fetch(url, {
      credentials: 'include',
      headers: buildRequestHeaders(true)
    });
    
    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Chat info API failed: ${response.status} ${text || ''}`.trim());
    }
    
    const data = await response.json();
    if (data && typeof data === 'object' && 'code' in data && 'data' in data) {
      return data.data || null;
    }
    return data || null;
  }
  
  async function fetchStudioCardInfo(sid) {
    if (!sid) return null;
    const url = `${API_BASE}/api/v1/studioCard/info?isguest=1&sid=${sid}`;
    console.log('[Arcaferry] Fetching studio card info from:', url);
    
    try {
      const response = await fetch(url, {
        credentials: 'include',
        headers: { 'Accept': 'application/json' }
      });
      
      if (!response.ok) return null;
      
      const data = await response.json();
      if (data?.code !== 0) return null;
      
      return data.data || null;
    } catch {
      return null;
    }
  }
  
  function extractBasicInfoFromStudioCard(apiData, sid) {
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

    const picture = (firstChar?.picture || apiData.picture || '');
    const avatarUrl = buildAvatarUrl(picture);

    return {
      basicInfo: {
        __source: 'share',
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
        creator: apiData.creator || apiData.author || '',
        author_name: apiData.author || apiData.authorName || '',
        creator_notes: apiData.creator_notes || apiData.creatorNotes || apiData.intro || firstChar?.intro || '',
        customAttrs: allAttrs,
        // Upstream may put greetings under prologue.greetings; keep prologue for mapping.
        greeting: apiData.greeting || [],
        prologue: apiData.prologue || null,
        intro: apiData.intro || firstChar?.intro || '',
        avatarUrl,
        characterbooks: apiData.characterbooks || [],
        shareId: sid,
        tags: apiData.tags || []
      },
      hiddenKeys,
      hiddenIndices
    };
  }
  
  function getHiddenAttrKeys(customAttrs) {
    if (!Array.isArray(customAttrs)) return [];

    return customAttrs
      .filter((attr) => attr && attr.isVisible === false)
      .map((attr) => attr.label || attr.name)
      .filter(Boolean);
  }
  
  function extractHiddenKeysFromChatInfo(chatInfo) {
    const keys = new Set();
    const charList = Array.isArray(chatInfo?.charList) ? chatInfo.charList : [];
    for (const char of charList) {
      const attrsList = [char.attrs, char.adviseAttrs, char.customAttrs]
        .filter(Array.isArray)
        .flat();
      for (const attr of attrsList) {
        if (attr && attr.isVisible === false) {
          const label = attr.label || attr.name;
          if (label) keys.add(label);
        }
      }
    }
    return Array.from(keys);
  }

  function extractHiddenFromChatInfo(chatInfo) {
    const firstChar = Array.isArray(chatInfo?.charList) ? chatInfo.charList[0] : null;
    const rootCustomLen = Array.isArray(chatInfo?.customAttrs) ? chatInfo.customAttrs.length : 0;
    const attrsLen = Array.isArray(firstChar?.attrs) ? firstChar.attrs.length : 0;
    const adviseLen = Array.isArray(firstChar?.adviseAttrs) ? firstChar.adviseAttrs.length : 0;
    const customStart = rootCustomLen + attrsLen + adviseLen;

    const customList = Array.isArray(firstChar?.customAttrs) ? firstChar.customAttrs : [];
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

    return { hiddenKeys: hiddenIndices.map((x) => x.label), hiddenIndices };
  }

  function buildBasicInfoFromChatInfo(chatInfo) {
    const firstChar = Array.isArray(chatInfo?.charList) ? chatInfo.charList[0] : null;

    const normalizeAttrs = (attrs) => {
      if (!Array.isArray(attrs)) return [];
      return attrs.map((a) => ({
        label: a.label || a.name,
        value: a.value,
        isVisible: a.isVisible
      }));
    };

    const allAttrs = [
      ...normalizeAttrs(chatInfo?.customAttrs),
      ...normalizeAttrs(firstChar?.attrs),
      ...normalizeAttrs(firstChar?.adviseAttrs),
      ...normalizeAttrs(firstChar?.customAttrs)
    ];

    const picture = firstChar?.picture || chatInfo?.picture || '';
    const avatarUrl = buildAvatarUrl(picture);

    return {
      __source: 'dream',
      // Match Pro (Rust) precedence: prefer charList[0].name when available.
      name: firstChar?.name || chatInfo?.name || '',
      description: chatInfo?.intro || chatInfo?.description || '',
      personality: chatInfo?.personality || '',
      scenario: chatInfo?.scenario || '',
      first_mes: chatInfo?.first_mes || chatInfo?.firstMes || '',
      // Match Pro (Rust): prefer chat_info.char_mes_example over info.mes_example.
      mes_example: chatInfo?.char_mes_example || chatInfo?.charMesExample || chatInfo?.mes_example || chatInfo?.mesExample || '',
      // Match Pro (Rust) precedence: prefer firstChar.prompt over info.system_prompt.
      system_prompt: firstChar?.prompt || chatInfo?.system_prompt || chatInfo?.systemPrompt || '',
      post_history_instructions: chatInfo?.post_history_instructions || chatInfo?.postHistoryInstructions || '',
      creator: chatInfo?.author || chatInfo?.creator || '',
      author_name: chatInfo?.author || chatInfo?.authorName || '',
      // Match Pro (Rust): prefer chat_info.char_creator_notes, then creator_notes, then intro.
      creator_notes: chatInfo?.char_creator_notes || chatInfo?.charCreatorNotes || chatInfo?.creator_notes || chatInfo?.creatorNotes || chatInfo?.intro || chatInfo?.description || '',
      customAttrs: allAttrs,
      greeting: chatInfo?.greeting || [],
      prologue: chatInfo?.prologue || null,
      intro: chatInfo?.intro || chatInfo?.description || '',
      // Preserve minimal chatInfo subtree needed by mapping parity.
      chatInfo: {
        studioPrologue: chatInfo?.studioPrologue || chatInfo?.studio_prologue || null,
        charMesExample: chatInfo?.charMesExample || chatInfo?.char_mes_example || null,
        charCreatorNotes: chatInfo?.charCreatorNotes || chatInfo?.char_creator_notes || null,
        studioCharList: chatInfo?.studioCharList || chatInfo?.studio_char_list || null,
        charCreatorNotesLegacy: chatInfo?.creatorNotes || chatInfo?.creator_notes || null,
        charMesExampleLegacy: chatInfo?.mesExample || chatInfo?.mes_example || null
      },
      avatarUrl,
      characterbooks: [],
      shareId: null,
      tags: chatInfo?.tags || []
    };
  }
  
  function getWorldbookFromShareState() {
    try {
      if (window.__ARCAFERRY_SHARE_STATE__?.basicInfo?.characterbooks) {
        const books = window.__ARCAFERRY_SHARE_STATE__.basicInfo.characterbooks;
        const entries = [];
        if (Array.isArray(books)) {
          for (const b of books) {
            const list = b.entryList || b.entry_list;
            if (Array.isArray(list)) entries.push(...list);
          }
        }
        return entries;
      }
    } catch {
    }
    return [];
  }

  async function extractWorldbook() {
    let { cid, index } = findCidAndIndexFromPage();
    let hiddenKeys = null;
    if (!index) index = findIndexFromUrl();

    if (!cid && capturedState.cid) cid = capturedState.cid;
    
    if (!cid || !index) {
      const captured = await waitForCaptured({
        needCid: !cid,
        needIndex: !index,
        timeoutMs: 4000
      });
      cid = cid || captured.cid;
      index = index || captured.index;
    }
    
    if ((!cid || !hiddenKeys) && index) {
      try {
        if (capturedChatInfo) {
          // Prefer chat cid over sid when both exist.
          setCapturedState({ cid: capturedChatInfo.cid || capturedChatInfo.sid, index });
          cid = cid || capturedState.cid;
          const extractedKeys = extractHiddenKeysFromChatInfo(capturedChatInfo);
          if (extractedKeys.length) {
            hiddenKeys = extractedKeys;
          }
        } else {
          const captured = await waitForCaptured({ needCid: !cid, needIndex: false, timeoutMs: 3000 });
          cid = cid || captured.cid;
        }
      } catch {
      }
    }
    
    if (!cid) {
      await captureChatInfoViaBackground(index);
      cid = cid || capturedState.cid;
    }
    
    if (!cid || !index) {
      const networkData = findFromNetworkRequests();
      cid = cid || networkData.cid;
      index = index || networkData.index;
    }
    
    console.log('[Arcaferry] Found - index:', index, 'cid:', cid);
    
    if (!index) {
      throw new Error('找不到 index。请确保页面已完全加载，或尝试刷新页面。');
    }
    
    if (!cid) {
      throw new Error('找不到 cid。请确保已开始对话（发送至少一条消息）。');
    }
    
    let worldbook = [];

    const embedded = getWorldbookFromShareState();
    // Prefer embedded share-state worldbook when it is NOT placeholder.
    if (embedded.length && !isPlaceholderEntries(embedded)) {
      worldbook = embedded;
    } else {
      worldbook = await fetchWorldbook(index, cid);
    }

    // IMPORTANT: Placeholders must never be persisted.
    if (worldbook.length > 0 && isPlaceholderEntries(worldbook)) {
      worldbook = [];
    }

    console.log('[Arcaferry] Worldbook entries (final):', worldbook.length);
    
    await chrome.runtime.sendMessage({
      action: 'saveWorldbook',
      data: {
        worldbook,
        cid,
        charIndex: index,
        hiddenKeys
      }
    });
    
    return { worldbook, hiddenKeys };
  }
  
  // Existing message listener logic
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.action === 'ping') {
      sendResponse({ success: true });
      return true;
    }

    if (message.action === 'extractWorldbook') {
      extractWorldbook().then(result => {
        sendResponse({ success: true, worldbook: result.worldbook, hiddenKeys: result.hiddenKeys });
      }).catch(err => {
        console.error('[Arcaferry] Worldbook extraction failed:', err);
        sendResponse({ success: false, error: err.message });
      });
      return true;
    }
    
    if (message.action === 'startExtraction') {
      (async () => {
        try {
          extractionEnabled = true;

          // Reset flags
          hasSharedStudioData = false;
          window.__ARCAFERRY_SHARE_STATE__ = null;
          
          await installInterceptorsViaBackground();

          // Give MAIN interceptors a brief moment to capture auth from any in-flight page requests.
          if (!capturedAuthToken) {
            const start = Date.now();
            while (!capturedAuthToken && Date.now() - start < 800) {
              await new Promise((r) => setTimeout(r, 100));
            }
          }
          
          // Ensure CID/index available (step C)
          let { cid, index } = findCidAndIndexFromPage();
          if (!index) index = findIndexFromUrl();
          if (!cid && capturedState.cid) cid = capturedState.cid;
          if (!index && capturedState.index) index = capturedState.index;
          
          if (!cid || !index) {
             const captured = await waitForCaptured({
               needCid: !cid,
               needIndex: !index,
               timeoutMs: 3000
             });
             cid = cid || captured.cid;
             index = index || captured.index;
          }
          
          if (!cid) {
            await captureChatInfoViaBackground(index);
            cid = cid || capturedState.cid;
          }
          
          // Step D: Basic info (must be explicit and authorized).
          // Use captureChatInfo (MAIN world XHR with Authorization from localStorage/sessionStorage scan).
          if (!capturedChatInfo && index) {
            await captureChatInfoViaBackground(index);
            const start = Date.now();
            while (!capturedChatInfo && Date.now() - start < 5000) {
              await new Promise((r) => setTimeout(r, 200));
            }
          }

          if (!capturedChatInfo) {
            throw new Error('无法获取聊天信息（可能未登录或会话不可用）。请确认已登录，并在对话中发送至少一条消息后重试。');
          }

          // Sync cid/index from captured chat info.
          if (capturedChatInfo.cid || capturedChatInfo.sid) {
            cid = cid || capturedChatInfo.cid || capturedChatInfo.sid;
            setCapturedState({ cid, index });
          }

          await handleCapturedChatInfo(capturedChatInfo);
          
          // Step E: Worldbook
          if (cid && index) {
            // Prefer page-captured worldbook (more reliable under some proxy/CF setups).
            let entries = await waitForWorldbookCapture(1500);
            if (entries.length > 0 && isPlaceholderEntries(entries)) entries = [];

            if (!entries.length) {
              const res = await fetchWorldbookViaBackground(index, cid);
              if (!res.success) {
                throw new Error(res.error || 'Worldbook API failed');
              }
              entries = res.entries || [];
            }

            // Success (even if empty) completes Step 2.
            await chrome.runtime.sendMessage({
              action: 'saveWorldbook',
              data: {
                worldbook: entries,
                cid,
                charIndex: index
              }
            });
          }
          
          sendResponse({ success: true });
          
        } catch (err) {
          console.error('[Arcaferry] Start extraction failed:', err);
          sendResponse({ success: false, error: err.message });
        }
      })();
      return true;
    }
    
    if (message.action === 'retryWorldbook') {
      (async () => {
         try {
           extractionEnabled = true;
           const { cid, index } = capturedState;
           if (!cid || !index) throw new Error('Missing cid/index for retry');

           let entries = await waitForWorldbookCapture(1500);
           if (entries.length > 0 && isPlaceholderEntries(entries)) entries = [];

           if (!entries.length) {
             const res = await fetchWorldbookViaBackground(index, cid);
             if (!res.success) {
               throw new Error(res.error || 'Worldbook API failed');
             }
             entries = res.entries || [];
           }

           await chrome.runtime.sendMessage({
             action: 'saveWorldbook',
             data: {
               worldbook: entries,
               cid,
               charIndex: index
             }
           });
           sendResponse({ success: true, count: entries.length });
         } catch (err) {
           sendResponse({ success: false, error: err.message });
         }
      })();
      return true;
    }
  });

  const originalFetch = window.fetch;
  const originalXhrOpen = XMLHttpRequest.prototype.open;
  const originalXhrSend = XMLHttpRequest.prototype.send;
  const originalXhrSetRequestHeader = XMLHttpRequest.prototype.setRequestHeader;
  
  XMLHttpRequest.prototype.open = function(method, url, ...rest) {
    this.__arcaferryUrl = url;
    return originalXhrOpen.call(this, method, url, ...rest);
  };

  XMLHttpRequest.prototype.setRequestHeader = function(header, value) {
    try {
      if (header && typeof header === 'string' && /authorization/i.test(header)) {
        const token = extractTokenFromValue(value);
        if (token && token !== capturedAuthToken) {
          capturedAuthToken = token;
        }
      }
    } catch {
    }
    return originalXhrSetRequestHeader.call(this, header, value);
  };
  
  XMLHttpRequest.prototype.send = function(...args) {
    this.addEventListener('load', function() {
      try {
        const url = this.__arcaferryUrl || '';
        if (!url) return;
        if (url.includes('info-by-chat-index')) {
          const payloadSource = this.responseType === 'json'
            ? this.response
            : JSON.parse(this.responseText || '{}');
          const data = (payloadSource && typeof payloadSource === 'object' && 'data' in payloadSource)
            ? payloadSource.data
            : payloadSource;
          if (data?.sid || data?.cid) {
            const payload = {
              // Prefer chat cid over sid when both exist.
              cid: valueToString(data.cid || data.sid),
              index: findIndexFromUrl() || capturedState.index
            };
            setCapturedState(payload);
            window.postMessage({ type: 'ARCAFERRY_CAPTURED', payload }, '*');
            console.log('[Arcaferry] Captured from xhr:', payload);
          } else if (payloadSource && payloadSource.code !== undefined) {
            console.warn('[Arcaferry] Chat info error response:', payloadSource.code);
          }
        }
      } catch (e) {}
    });
    return originalXhrSend.apply(this, args);
  };
  
  function tryWarmupChatInfo(index) {
    if (!index) return;
    const url = `${API_BASE}/api/v1/user/character/info-by-chat-index?index=${index}`;
    fetch(url, {
      credentials: 'include',
      headers: buildRequestHeaders(true)
    }).catch(() => {});
  }
  
  // Strong manual mode: do not use background fetchChatInfo (it cannot reliably attach Authorization).
  
  async function captureChatInfoViaBackground(index) {
    const tabId = await new Promise((resolve) => {
      chrome.runtime.sendMessage({ action: 'getTabId' }, (resp) => {
        if (resp?.tabId) resolve(resp.tabId);
        else resolve(null);
      });
    });
    if (!tabId) return;
    await chrome.runtime.sendMessage({
      action: 'captureChatInfo',
      data: { tabId, index, apiBase: API_BASE, authToken: capturedAuthToken }
    });
    const captured = await waitForCaptured({ needCid: true, needIndex: false, timeoutMs: 5000 });
    if (captured?.cid) {
      capturedState.cid = captured.cid;
    }
  }

  function installInterceptorsViaBackground() {
    return new Promise((resolve) => {
      try {
        chrome.runtime.sendMessage(
          {
            action: 'installInterceptors',
            data: { apiBase: API_BASE }
          },
          () => resolve(true)
        );
      } catch {
        resolve(false);
      }
    });
  }
  
  window.fetch = async function(...args) {
    const init = args[1];
    if (init && init.headers) {
      const authHeader = init.headers.Authorization || init.headers.authorization;
      if (authHeader && typeof authHeader === 'string') {
        const token = extractTokenFromValue(authHeader);
        if (token && token !== capturedAuthToken) {
          capturedAuthToken = token;
        }
      }
    }
    const response = await originalFetch.apply(this, args);
    
    try {
      const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
      
      if (url.includes('info-by-chat-index')) {
        const clonedResponse = response.clone();
        clonedResponse.json().then(data => {
          const payloadSource = (data && typeof data === 'object' && 'data' in data) ? data.data : data;
          if (payloadSource?.sid || payloadSource?.cid) {
            const payload = {
              // Prefer chat cid over sid when both exist.
              cid: valueToString(payloadSource.cid || payloadSource.sid),
              index: findIndexFromUrl() || capturedState.index
            };
            setCapturedState(payload);
            window.postMessage({ type: 'ARCAFERRY_CAPTURED', payload }, '*');
            console.log('[Arcaferry] Captured from fetch:', payload);
          }
        }).catch(() => {});
      }
      
      if (url.includes('studioCard/info')) {
        // IMPORTANT: do NOT overwrite dream chat cid/index from studioCard/info.
        // studioCard/info may contain a different cid than the dream chat session.
      }
    } catch (e) {}
    
    return response;
  };
  
  console.log('[Arcaferry] Build:', ARCAFERRY_BUILD);
  
  // Strong manual mode: we still install MAIN interceptors on load (PASSIVE).
  // They only capture token/cid/worldbook into in-memory variables, and DO NOT write storage.
  // This avoids missing early authenticated requests while keeping extraction Start-gated.
  installInterceptorsViaBackground();
  
  // const warmupOnce = () => { ... }
  // if (!warmupOnce()) { ... }
  
  console.log('[Arcaferry] Chat page script loaded (manual mode)');
  
})();
