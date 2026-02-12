const STORAGE_KEY = 'currentCard';

const DEFAULT_STATE = {
  step: 0,
  basicInfo: null,
  worldbook: null,
  hiddenKeys: [],
  hiddenIndices: [],
  hiddenSettings: null,
  ccv3Card: null,
  shareId: null,
  cid: null,
  charIndex: null
};

chrome.runtime.onInstalled.addListener(async () => {
  await chrome.storage.local.set({ [STORAGE_KEY]: DEFAULT_STATE });
});

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  handleMessage(message, sender).then(sendResponse);
  return true;
});

async function handleMessage(message, sender) {
  switch (message.action) {
    case 'saveBasicInfo':
      return await saveBasicInfo(message.data);
    
    case 'saveWorldbook':
      return await saveWorldbook(message.data);
    
    case 'getState':
      return await getState();
    
    case 'updateState':
      return await updateState(message.data);
    
    case 'reset':
      return await resetState();
    
    case 'fetchChatInfo':
      return await fetchChatInfo(message.data);
    
    case 'captureChatInfo':
      return await captureChatInfo(message.data);
    
    case 'installInterceptors':
      return await installInterceptors(message.data, sender);
    
    case 'fetchWorldbook':
      return await fetchWorldbook(message.data);

    
    case 'getTabId':
      return { success: true, tabId: sender?.tab?.id || null };
    
    default:
      return { success: false, error: 'Unknown action' };
  }
}

async function getState() {
  const result = await chrome.storage.local.get([STORAGE_KEY]);
  return { success: true, state: result[STORAGE_KEY] || DEFAULT_STATE };
}

async function updateState(updates) {
  const current = await getState();
  const newState = { ...current.state, ...updates };
  await chrome.storage.local.set({ [STORAGE_KEY]: newState });
  return { success: true, state: newState };
}

async function resetState() {
  await chrome.storage.local.set({ [STORAGE_KEY]: DEFAULT_STATE });
  return { success: true };
}

function mergeBasicInfo(prev, next) {
  const merged = { ...(prev || {}) };
  const srcPrev = merged.__source || '';
  const srcNext = next?.__source || '';

  const prevIsShare = srcPrev === 'share';
  const nextIsShare = srcNext === 'share';
  
  const preferNext = !prev || !prevIsShare || nextIsShare;

  for (const [k, v] of Object.entries(next || {})) {
    if (v === undefined || v === null) continue;
    if (typeof v === 'string' && v.trim() === '') continue;
    if (Array.isArray(v) && v.length === 0) continue;

    if (preferNext) {
      merged[k] = v;
    } else {
      const existing = merged[k];
      const hasExisting = !(existing === undefined || existing === null || (typeof existing === 'string' && existing.trim() === '') || (Array.isArray(existing) && existing.length === 0));
      if (!hasExisting) merged[k] = v;
    }
  }

  merged.__source = prevIsShare ? 'share' : (srcNext || srcPrev || 'dream');
  return merged;
}

async function saveBasicInfo(data) {
  const { basicInfo, hiddenKeys, hiddenIndices, shareId } = data;

  const current = await getState();

  const nextInfo = mergeBasicInfo(current.state.basicInfo, basicInfo);

  const nextHiddenIndices = Array.isArray(hiddenIndices) && hiddenIndices.length
    ? hiddenIndices
    : (current.state.hiddenIndices || []);

  const nextHiddenKeys = Array.isArray(nextHiddenIndices) && nextHiddenIndices.length
    ? nextHiddenIndices.map((x) => x?.label).filter(Boolean)
    : Array.from(new Set([...(current.state.hiddenKeys || []), ...(hiddenKeys || [])]));

  const newState = {
    ...current.state,
    step: Math.max(current.state.step || 0, 2),
    basicInfo: nextInfo,
    hiddenKeys: nextHiddenKeys,
    hiddenIndices: nextHiddenIndices,
    shareId: shareId ?? current.state.shareId
  };

  await chrome.storage.local.set({ [STORAGE_KEY]: newState });

  notifyPopup('cardDataExtracted', { basicInfo: newState.basicInfo, hiddenKeys: newState.hiddenKeys, hiddenIndices: newState.hiddenIndices });

  return { success: true };
}

async function saveWorldbook(data) {
  const { worldbook, cid, charIndex, hiddenKeys } = data;
  
  const current = await getState();
  const newState = {
    ...current.state,
    step: 3,
    worldbook: worldbook || [],
    cid,
    charIndex,
    hiddenKeys: hiddenKeys || current.state.hiddenKeys || []
  };
  
  await chrome.storage.local.set({ [STORAGE_KEY]: newState });
  
  notifyPopup('worldbookExtracted', { worldbook, hiddenKeys: newState.hiddenKeys, hiddenIndices: newState.hiddenIndices });
  
  return { success: true };
}

async function fetchChatInfo(data) {
  const index = data?.index;
  if (!index) return { success: false, error: 'Missing index' };
  const apiBase = data?.apiBase || 'https://quack.im';
  const url = `${apiBase}/api/v1/user/character/info-by-chat-index?index=${index}`;
  try {
    const response = await fetch(url, {
      credentials: 'include',
      headers: {
        'Accept': 'application/json, text/plain, */*',
        'X-Requested-With': 'XMLHttpRequest'
      }
    });
    if (!response.ok) {
      const text = await response.text();
      return { success: false, error: `Chat info API failed: ${response.status} ${text || ''}`.trim() };
    }
    const data = await response.json();
    if (data && typeof data === 'object' && 'code' in data && 'data' in data) {
      return { success: true, data: data.data || null };
    }
    return { success: true, data };
  } catch (error) {
    return { success: false, error: error?.message || 'Chat info API failed' };
  }
}

async function fetchWorldbook(data) {
  const tabId = data?.tabId;
  const index = data?.index;
  const cid = data?.cid;
  const apiBase = data?.apiBase || 'https://quack.im';
  const authToken = data?.authToken || '';
  if (!tabId) return { success: false, error: 'Missing tabId' };
  if (!index) return { success: false, error: 'Missing index' };
  if (!cid) return { success: false, error: 'Missing cid' };

  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      world: 'MAIN',
      args: [apiBase, index, cid, authToken],
      func: async (apiBaseValue, indexValue, cidValue, authTokenValue) => {
        // Use same-origin relative URL in MAIN world (more consistent with page requests).
        const url = `/api/v1/chat/getCharacterBooks?index=${indexValue}&cid=${cidValue}`;

        const getCookieMap = () => {
          const map = {};
          const raw = document.cookie || '';
          raw.split(';').forEach(pair => {
            const idx = pair.indexOf('=');
            if (idx === -1) return;
            const key = pair.slice(0, idx).trim();
            const value = pair.slice(idx + 1).trim();
            if (key) map[key.toLowerCase()] = decodeURIComponent(value);
          });
          return map;
        };

        const getCsrfTokenFromCookies = () => {
          const cookies = getCookieMap();
          const keys = ['xsrf-token', 'x-csrf-token', 'csrf-token', 'csrftoken', 'csrf'];
          for (const key of keys) {
            if (cookies[key]) return cookies[key];
          }
          return '';
        };

        const readToken = () => {
          const jwtPattern = /^[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$/;
          const extractJwt = (value) => {
            if (!value || typeof value !== 'string') return '';
            const trimmed = value.trim();
            if (!trimmed) return '';
            const raw = trimmed.startsWith('Bearer ') ? trimmed.slice(7) : trimmed;
            return jwtPattern.test(raw) ? raw : '';
          };

          // Prefer token captured from page requests (passed in from content script).
          const explicit = extractJwt(authTokenValue);
          if (explicit) return explicit;

          const scan = (store) => {
            try {
              for (let i = 0; i < store.length; i += 1) {
                const key = store.key(i);
                if (!key) continue;
                const value = store.getItem(key);
                if (!value) continue;
                if (/token|auth/i.test(key)) {
                  const token = extractJwt(value);
                  if (token) return token;
                }
                const direct = extractJwt(value);
                if (direct) return direct;
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
                      const token = extractJwt(candidate);
                      if (token) return token;
                    }
                  }
                } catch {
                }
              }
            } catch {
            }
            return '';
          };
          return scan(window.localStorage) || scan(window.sessionStorage);
        };

        const headers = {
          'Accept': 'application/json, text/plain, */*',
          'X-Requested-With': 'XMLHttpRequest'
        };
        const csrfToken = getCsrfTokenFromCookies();
        if (csrfToken) {
          headers['X-CSRF-Token'] = csrfToken;
          headers['X-XSRF-TOKEN'] = csrfToken;
        }
        const token = readToken();
        if (token) headers['Authorization'] = `Bearer ${token}`;

        const resp = await fetch(url, {
          credentials: 'include',
          headers
        });

        const contentType = resp.headers.get('content-type') || '';
        const text = await resp.text().catch(() => '');

        if (!resp.ok) {
          return { ok: false, status: resp.status, text };
        }

        // Some environments/proxies may return JSON with an incorrect Content-Type.
        // Attempt JSON parse regardless of Content-Type.
        let json;
        try {
          json = JSON.parse(text);
        } catch {
          return { ok: false, status: resp.status, text: `non-json:${contentType} ${text}`.trim() };
        }

        if (json?.code !== 0) {
          return { ok: false, status: 200, text: json?.msg || 'Worldbook API error' };
        }

        const entries = [];
        if (Array.isArray(json.data)) {
          for (const book of json.data) {
            if (Array.isArray(book.entryList)) entries.push(...book.entryList);
          }
        }

        return { ok: true, entries };
      }
    });

    const result = results?.[0]?.result;
    if (!result?.ok) {
      return { success: false, error: `Worldbook API failed: ${result?.status || ''} ${result?.text || ''}`.trim() };
    }

    return { success: true, entries: result.entries || [] };
  } catch (error) {
    return { success: false, error: error?.message || 'Worldbook API failed' };
  }
}

async function captureChatInfo(data) {
  const tabId = data?.tabId;
  if (!tabId) return { success: false, error: 'Missing tabId' };
  const index = data?.index;
  if (!index) return { success: false, error: 'Missing index' };
  const apiBase = data?.apiBase || 'https://quack.im';
  const authToken = data?.authToken || '';
  try {
    await chrome.scripting.executeScript({
      target: { tabId },
      world: 'MAIN',
      args: [apiBase, index, authToken],
      func: (apiBaseValue, indexValue, authTokenValue) => {
        try {
          const readToken = () => {
            const jwtPattern = /^[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$/;
            const extractJwt = (value) => {
              if (!value || typeof value !== 'string') return '';
              const trimmed = value.trim();
              if (!trimmed) return '';
              const raw = trimmed.startsWith('Bearer ') ? trimmed.slice(7) : trimmed;
              return jwtPattern.test(raw) ? raw : '';
            };

            const explicit = extractJwt(authTokenValue);
            if (explicit) return explicit;
            const scan = (store) => {
              try {
                for (let i = 0; i < store.length; i += 1) {
                  const key = store.key(i);
                  if (!key) continue;
                  const value = store.getItem(key);
                  if (!value) continue;
                  if (/token|auth/i.test(key)) {
                    const token = extractJwt(value);
                    if (token) return token;
                  }
                  const direct = extractJwt(value);
                  if (direct) return direct;
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
                        const token = extractJwt(candidate);
                        if (token) return token;
                      }
                    }
                  } catch {
                  }
                }
              } catch {
              }
              return '';
            };
            return scan(window.localStorage) || scan(window.sessionStorage);
          };
          // Use same-origin relative URL in MAIN world.
          const url = `/api/v1/user/character/info-by-chat-index?index=${indexValue}`;
          const xhr = new XMLHttpRequest();
          xhr.open('GET', url, true);
          xhr.responseType = 'json';
          xhr.withCredentials = true;
          try {
            xhr.setRequestHeader('Accept', 'application/json, text/plain, */*');
            xhr.setRequestHeader('X-Requested-With', 'XMLHttpRequest');
          } catch {
          }
          const token = readToken();
          if (token) {
            xhr.setRequestHeader('Authorization', `Bearer ${token}`);
          }
          xhr.onload = () => {
            try {
              const payloadSource = xhr.response || {};
              const data = (payloadSource && typeof payloadSource === 'object' && 'data' in payloadSource)
                ? payloadSource.data
                : payloadSource;
               if (data?.sid || data?.cid) {
                 window.postMessage({
                   type: 'ARCAFERRY_CAPTURED',
                   payload: {
                     // Prefer chat cid over sid when both exist.
                     cid: data.cid || data.sid,
                     index: indexValue
                   },
                   chatInfo: data
                 }, '*');
               }
            } catch {
            }
          };
          xhr.send();
        } catch {
        }
      }
    });
    return { success: true };
  } catch (error) {
    return { success: false, error: error?.message || 'Capture failed' };
  }
}

async function installInterceptors(data, sender) {
  const tabId = data?.tabId || sender?.tab?.id;
  if (!tabId) return { success: false, error: 'Missing tabId' };
  const apiBase = data?.apiBase || 'https://quack.im';
  try {
    await chrome.scripting.executeScript({
      target: { tabId },
      world: 'MAIN',
      args: [apiBase],
      func: (apiBaseValue) => {
        if (window.__ARCAFERRY_INTERCEPTOR_INSTALLED__) return;
        window.__ARCAFERRY_INTERCEPTOR_INSTALLED__ = true;
        try { console.log('[Arcaferry] MAIN interceptor installed'); } catch {}

        const extractToken = (value) => {
          if (!value || typeof value !== 'string') return '';
          const trimmed = value.trim();
          if (!trimmed) return '';
          if (trimmed.startsWith('Bearer ')) return trimmed.slice(7);
          const jwtPattern = /^[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$/;
          if (jwtPattern.test(trimmed)) return trimmed;
          return '';
        };

        const emitAuth = (tokenValue) => {
          const token = extractToken(tokenValue);
          if (!token) return;
          // Do NOT log tokens.
          try {
            if (window.__ARCAFERRY_AUTH_TOKEN__ !== token) {
              window.__ARCAFERRY_AUTH_TOKEN__ = token;
              window.postMessage({ type: 'ARCAFERRY_AUTH', payload: { token } }, '*');
            }
          } catch {
          }
        };

        const getIndex = () => {
          try {
            const url = new URL(window.location.href);
            const pathMatch = url.pathname.match(/\/dream\/([^/?]+)/);
            if (pathMatch) return pathMatch[1];
            return url.searchParams.get('index') || '';
          } catch {
            return '';
          }
        };
        const parseUrl = (u) => {
          try {
            return new URL(u, window.location.origin);
          } catch {
            return null;
          }
        };

        const emitCaptured = (data) => {
          if (data?.sid || data?.cid) {
            window.postMessage({
              type: 'ARCAFERRY_CAPTURED',
              payload: {
                // Prefer chat cid over sid when both exist.
                cid: data.cid || data.sid,
                index: getIndex()
              },
              chatInfo: data
            }, '*');
          }
        };

        const emitWorldbook = (urlValue, payloadSource) => {
          if (!payloadSource || typeof payloadSource !== 'object') return;
          
          // API 响应格式: { code: 0, data: [...books...] }
          if (payloadSource.code !== 0) return;
          
          const books = payloadSource.data;
          if (!Array.isArray(books)) return;

          const entries = [];
          for (const book of books) {
            if (Array.isArray(book.entryList)) entries.push(...book.entryList);
          }

          const parsed = parseUrl(urlValue);
          const cid = parsed?.searchParams?.get('cid') || '';
          const index = parsed?.searchParams?.get('index') || getIndex();

          if (!entries.length) return;

          window.postMessage({
            type: 'ARCAFERRY_WORLDBOOK',
            payload: { worldbook: entries, cid, index }
          }, '*');
        };

        const handleChatInfoResponse = (payloadSource) => {
          const data = (payloadSource && typeof payloadSource === 'object' && 'data' in payloadSource)
            ? payloadSource.data
            : payloadSource;
          emitCaptured(data);
        };
        const originalFetch = window.fetch;
        window.fetch = async function(...args) {
          const requestUrl = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
          try {
            const init = args[1];
            const headers = init?.headers;
            const authHeader = headers?.Authorization || headers?.authorization;
            if (typeof authHeader === 'string') emitAuth(authHeader);
          } catch {
          }
          const response = await originalFetch.apply(this, args);
          try {
            if (requestUrl.includes('info-by-chat-index')) {
              try { console.log('[Arcaferry] MAIN saw fetch:', requestUrl); } catch {}
              response.clone().json().then(handleChatInfoResponse).catch(() => {});
            }
            if (requestUrl.includes('getCharacterBooks')) {
              try { console.log('[Arcaferry] MAIN saw fetch:', requestUrl); } catch {}
              response.clone().json().then((json) => emitWorldbook(requestUrl, json)).catch(() => {});
            }
          } catch {
          }
          return response;
        };
        const originalOpen = XMLHttpRequest.prototype.open;
        const originalSend = XMLHttpRequest.prototype.send;
        const originalSetHeader = XMLHttpRequest.prototype.setRequestHeader;
        XMLHttpRequest.prototype.open = function(method, url, ...rest) {
          this.__arcaferryUrl = url;
          return originalOpen.call(this, method, url, ...rest);
        };
        XMLHttpRequest.prototype.setRequestHeader = function(header, value) {
          try {
            if (header && typeof header === 'string' && /authorization/i.test(header)) {
              emitAuth(value);
            }
          } catch {
          }
          return originalSetHeader.call(this, header, value);
        };
        XMLHttpRequest.prototype.send = function(...args) {
          this.addEventListener('load', function() {
            try {
              const url = this.__arcaferryUrl || '';
              if (url.includes('info-by-chat-index')) {
                try { console.log('[Arcaferry] MAIN saw xhr:', url, this.status); } catch {}
                const payloadSource = this.responseType === 'json'
                  ? this.response
                  : JSON.parse(this.responseText || '{}');
                handleChatInfoResponse(payloadSource);
              }
              if (url.includes('getCharacterBooks')) {
                try { console.log('[Arcaferry] MAIN saw xhr:', url, this.status); } catch {}
                const payloadSource = this.responseType === 'json'
                  ? this.response
                  : JSON.parse(this.responseText || '{}');
                emitWorldbook(url, payloadSource);
              }
            } catch {
            }
          });
          return originalSend.apply(this, args);
        };
        try {
          // Manual mode: do NOT auto-fetch info-by-chat-index on install
          /*
          const url = `/api/v1/user/character/info-by-chat-index?index=${getIndex()}`;
          const xhr = new XMLHttpRequest();
          xhr.open('GET', url, true);
          xhr.responseType = 'json';
          xhr.withCredentials = true;
          xhr.onload = () => {
            try {
              handleChatInfoResponse(xhr.response || {});
            } catch {
            }
          };
          xhr.send();
          */
        } catch {
        }
      }
    });
    return { success: true };
  } catch (error) {
    return { success: false, error: error?.message || 'Install failed' };
  }
}

function notifyPopup(action, data) {
  chrome.runtime.sendMessage({ action, data }).catch(() => {
  });
}

chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.status !== 'complete') return;
  
  const url = tab.url || '';
  
  if (/quack\.im\/discovery\/share\//.test(url)) {
    chrome.action.setBadgeText({ text: '1', tabId });
    chrome.action.setBadgeBackgroundColor({ color: '#e94560', tabId });
  } else if (/quack\.im\/dream\//.test(url)) {
    chrome.action.setBadgeText({ text: '2', tabId });
    chrome.action.setBadgeBackgroundColor({ color: '#4ade80', tabId });
  } else {
    chrome.action.setBadgeText({ text: '', tabId });
  }
});
