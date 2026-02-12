import { mapQuackToV3, serializeCard } from '../lib/ccv3.js';
import { createCardPng, downloadBlob } from '../lib/png-embed.js';
import {
  parseHiddenSettingsResponse,
  replacePlaceholders,
  generateInductionPrompt as buildInductionPrompt
} from '../lib/parser.js';

const STORAGE_KEY = 'currentCard';
const THEME_KEY = 'arcaferry-theme';

const elements = {};

let currentState = {
  step: 0,
  pageType: 'unknown',
  theme: 'system',
  basicInfo: null,
  worldbook: null,
  hiddenKeys: [],
  hiddenIndices: [],
  hiddenSettings: null,
  ccv3Card: null
};

let pageCheckInterval = null;

const TOAST_ICONS = {
  success: `
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <polyline points="20 6 9 17 4 12"></polyline>
    </svg>
  `,
  error: `
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <line x1="18" y1="6" x2="6" y2="18"></line>
      <line x1="6" y1="6" x2="18" y2="18"></line>
    </svg>
  `,
  warning: `
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
      <line x1="12" y1="9" x2="12" y2="13"></line>
      <line x1="12" y1="17" x2="12.01" y2="17"></line>
    </svg>
  `,
  info: `
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9"></circle>
      <line x1="12" y1="10" x2="12" y2="16"></line>
      <line x1="12" y1="7" x2="12.01" y2="7"></line>
    </svg>
  `
};

function initElements() {
  const ids = [
    'themeToggle', 'themeIconLight', 'themeIconDark', 'themeIconSystem',
    'progressText', 'progressStep', 'progressFill',
    'startBtn', 'pageTypeHint',
    'guideStatusText',
    'inductionPrompt', 'copyPromptBtn', 'aiResponse', 'parseResponseBtn',
    'parsedResult', 'parsedCount', 'skipStep3Btn',
    'avatarPreview', 'cardName', 'cardMeta',
    'attrCount', 'worldbookCount', 'hiddenCount',
    'downloadPngBtn', 'downloadJsonBtn', 'copyJsonBtn', 'restartBtn',
    'resetBtn', 'toast', 'toastIcon', 'toastMessage'
  ];
  
  ids.forEach(id => {
    elements[id] = document.getElementById(id);
  });
}

function bindEvents() {
  elements.themeToggle?.addEventListener('click', handleThemeToggle);
  elements.startBtn?.addEventListener('click', handleStart);
  elements.resetBtn?.addEventListener('click', () => handleReset());
  elements.restartBtn?.addEventListener('click', () => handleReset());
  elements.copyPromptBtn?.addEventListener('click', handleCopyPrompt);
  elements.parseResponseBtn?.addEventListener('click', handleParseResponse);
  elements.skipStep3Btn?.addEventListener('click', handleSkipStep3);
  elements.downloadPngBtn?.addEventListener('click', handleDownloadPng);
  elements.downloadJsonBtn?.addEventListener('click', handleDownloadJson);
  elements.copyJsonBtn?.addEventListener('click', handleCopyJson);
}

async function loadState() {
  const result = await chrome.storage.local.get([STORAGE_KEY]);
  if (result[STORAGE_KEY]) {
    const saved = result[STORAGE_KEY];
    currentState = { ...currentState, ...saved };
  }
  
  const savedTheme = localStorage.getItem(THEME_KEY) || 'system';
  setTheme(savedTheme);
}

async function saveState() {
  await chrome.storage.local.set({ [STORAGE_KEY]: currentState });
}

function setTheme(theme) {
  currentState.theme = theme;
  
  const icons = ['themeIconLight', 'themeIconDark', 'themeIconSystem'];
  icons.forEach((id) => {
    elements[id]?.classList.add('hidden');
  });
  
  let activeIcon;
  if (theme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.setAttribute('data-theme', prefersDark ? 'dark' : 'light');
    activeIcon = 'themeIconSystem';
  } else {
    document.documentElement.setAttribute('data-theme', theme);
    activeIcon = theme === 'dark' ? 'themeIconDark' : 'themeIconLight';
  }
  
  elements[activeIcon]?.classList.remove('hidden');
  localStorage.setItem(THEME_KEY, theme);
}

function handleThemeToggle() {
  const themes = ['light', 'dark', 'system'];
  const currentIndex = themes.indexOf(currentState.theme);
  const nextTheme = themes[(currentIndex + 1) % themes.length];
  setTheme(nextTheme);
  showToast(`主题: ${nextTheme === 'system' ? '跟随系统' : nextTheme === 'dark' ? '深色' : '浅色'}`, 'info');
}

window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
  if (currentState.theme === 'system') {
    document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light');
  }
});

async function detectPageType() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.url) return 'unknown';
  
  const isShare = /quack\.im\/discovery\/share\//.test(tab.url);
  const isDream = /quack\.im\/dream\//.test(tab.url);
  
  if (isShare) return 'share';
  if (isDream) return 'dream';
  return 'unsupported';
}

function updateProgress(stepNum) {
  const percentage = (stepNum / 4) * 100;
  elements.progressFill.style.width = `${percentage}%`;
  elements.progressStep.textContent = `步骤 ${stepNum}/4`;
  
  const texts = {
    0: '准备就绪',
    1: '开始提取',
    2: '提取基础信息',
    3: '提取世界书',
    4: '提取隐藏设定',
    5: '完成'
  };
  elements.progressText.textContent = texts[stepNum] || '处理中';
}

function switchToStage(stageNum) {
  document.querySelectorAll('.stage').forEach(stage => {
    stage.classList.remove('active');
  });
  
  const targetStage = document.querySelector(`.stage-${stageNum}`);
  if (targetStage) {
    targetStage.classList.add('active');
  }
  
  updateProgress(stageNum);
  currentState.step = stageNum;
  saveState();
  
  if (stageNum === 3) {
    initStage3();
  } else if (stageNum === 4) {
    initStage4();
  }
}

function updatePageTypeHint(pageType) {
  const hints = {
    share: '检测到 Share 页面，点击开始提取',
    dream: '检测到对话页面，点击开始提取',
    unsupported: '未检测到可提取页面，请先进入 Quack 对话或分享页面'
  };
  elements.pageTypeHint.textContent = hints[pageType] || hints.unsupported;
  elements.pageTypeHint.style.color = pageType === 'unsupported' ? 'var(--error)' : 'var(--text-muted)';
}

async function handleStart() {
  const pageType = await detectPageType();
  currentState.pageType = pageType;
  
  if (pageType === 'unsupported') {
    showToast('未检测到可提取页面，请先进入 Quack 对话/分享页面', 'error');
    return;
  }
  
  await chrome.runtime.sendMessage({ action: 'reset' });
  
  if (pageType === 'share') {
    await startShareExtraction();
  } else if (pageType === 'dream') {
    await startDreamExtraction();
  }
}

async function startShareExtraction() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) return;
  
  switchToStage(2);
  
  chrome.tabs.sendMessage(tab.id, { action: 'extractBasicInfo' }, (response) => {
    if (chrome.runtime.lastError) {
      showToast('无法连接到页面，请刷新', 'error');
      return;
    }
    if (!response?.success) {
      showToast(response?.error || '提取失败', 'error');
    }
  });
  
  startPageCheck();
}

async function startDreamExtraction() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) return;
  
  switchToStage(2);
  elements.progressText.textContent = '提取中...';
  
  chrome.tabs.sendMessage(tab.id, { action: 'startExtraction' }, (response) => {
    if (chrome.runtime.lastError) {
      showToast('无法连接到页面，请刷新', 'error');
      return;
    }
    if (!response?.success) {
      showToast(response?.error || '提取失败', 'error');
    }
  });
}

function startPageCheck() {
  if (pageCheckInterval) {
    clearInterval(pageCheckInterval);
  }
  
  pageCheckInterval = setInterval(async () => {
    const pageType = await detectPageType();
    if (pageType === 'dream') {
      clearInterval(pageCheckInterval);
      pageCheckInterval = null;
      elements.guideStatusText.textContent = '已进入对话页面，继续提取...';
      await startDreamExtraction();
    }
  }, 2000);
}

function initStage3() {
  const indices = Array.isArray(currentState.hiddenIndices) ? currentState.hiddenIndices : [];
  const keys = indices.length ? indices.map(x => x?.label).filter(Boolean) : currentState.hiddenKeys;
  const prompt = buildInductionPrompt(keys);
  elements.inductionPrompt.value = prompt;
  
  if (currentState.hiddenSettings) {
    elements.parsedResult.classList.remove('hidden');
    elements.parsedCount.textContent = Object.keys(currentState.hiddenSettings).length;
  } else {
    elements.parsedResult.classList.add('hidden');
  }
  
  const hasHidden = currentState.hiddenKeys.length > 0 || currentState.hiddenIndices.length > 0;
  if (!hasHidden) {
    elements.skipStep3Btn.textContent = '无隐藏设定，继续';
  } else {
    elements.skipStep3Btn.textContent = '跳过此步骤';
  }
}

function initStage4() {
  const info = currentState.basicInfo;
  if (!info) return;
  
  elements.avatarPreview.src = info.avatarUrl || '';
  elements.cardName.textContent = info.name || '未知角色';
  
  const attrCount = (info.customAttrs || []).length;
  const worldbookCount = (currentState.worldbook || []).length;
  const hiddenCount = currentState.hiddenSettings ? Object.keys(currentState.hiddenSettings).length : 0;
  
  elements.attrCount.textContent = attrCount;
  elements.worldbookCount.textContent = worldbookCount;
  elements.hiddenCount.textContent = hiddenCount;
}

async function handleCopyPrompt() {
  try {
    await navigator.clipboard.writeText(elements.inductionPrompt.value);
    showToast('已复制提示词', 'success');
  } catch {
    showToast('复制失败', 'error');
  }
}

async function handleParseResponse() {
  const response = elements.aiResponse.value.trim();
  if (!response) {
    showToast('请粘贴 AI 回复', 'error');
    return;
  }
  
  const indices = Array.isArray(currentState.hiddenIndices) ? currentState.hiddenIndices : [];
  const keys = indices.length ? indices.map(x => x?.label).filter(Boolean) : currentState.hiddenKeys;
  const personaName = currentState.basicInfo?.personaName || '';
  console.log('[Arcaferry] Parsing hidden response with personaName:', personaName || '(empty)');
  const parsed = parseHiddenSettingsResponse(response, keys, indices, personaName);
  
  if (parsed.size === 0) {
    showToast('未能解析出隐藏设定', 'error');
    return;
  }
  
  currentState.hiddenSettings = Object.fromEntries(parsed);
  
  if (currentState.basicInfo?.customAttrs && indices.length) {
    for (const item of indices) {
      const { attrIndex, label, mergedIndex } = item;
      if (typeof attrIndex !== 'number' || !label || typeof mergedIndex !== 'number') continue;
      
      const value = currentState.hiddenSettings[`${attrIndex}:${label}`];
      if (!value) continue;
      
      const target = currentState.basicInfo.customAttrs[mergedIndex];
      if (!target || target.isVisible !== false) continue;
      target.value = value;
    }
  }
  
  buildCard();
  await saveState();
  switchToStage(4);
  showToast(`解析成功: ${parsed.size} 个设定`, 'success');
}

async function handleSkipStep3() {
  buildCard();
  await saveState();
  switchToStage(4);
}

function buildCard() {
  if (!currentState.basicInfo) return;
  
  let info = { ...currentState.basicInfo };
  
  if (currentState.hiddenSettings) {
    const valuesByLabel = new Map();
    for (const [k, v] of Object.entries(currentState.hiddenSettings)) {
      const parts = String(k).split(':');
      const label = parts.length >= 2 ? parts.slice(1).join(':') : String(k);
      if (!valuesByLabel.has(label)) valuesByLabel.set(label, v);
    }
    
    if (info.description) {
      info.description = replacePlaceholders(info.description, valuesByLabel);
    }
    if (info.system_prompt) {
      info.system_prompt = replacePlaceholders(info.system_prompt, valuesByLabel);
    }
  }
  
  currentState.ccv3Card = mapQuackToV3(info, currentState.worldbook || []);
}

async function handleDownloadPng() {
  if (!currentState.ccv3Card || !currentState.basicInfo?.avatarUrl) {
    showToast('数据不完整', 'error');
    return;
  }
  
  try {
    const blob = await createCardPng(currentState.basicInfo.avatarUrl, currentState.ccv3Card);
    const filename = `${currentState.ccv3Card.data.name.replace(/[/\\?%*:|"<>]/g, '_')}.png`;
    downloadBlob(blob, filename);
    showToast('下载成功', 'success');
  } catch (err) {
    console.error('Download PNG error:', err);
    showToast('下载失败: ' + err.message, 'error');
  }
}

function handleDownloadJson() {
  if (!currentState.ccv3Card) {
    showToast('数据不完整', 'error');
    return;
  }
  
  const json = serializeCard(currentState.ccv3Card);
  const blob = new Blob([json], { type: 'application/json' });
  const filename = `${currentState.ccv3Card.data.name.replace(/[/\\?%*:|"<>]/g, '_')}.json`;
  
  downloadBlob(blob, filename);
  showToast('下载成功', 'success');
}

async function handleCopyJson() {
  if (!currentState.ccv3Card) {
    showToast('数据不完整', 'error');
    return;
  }
  
  try {
    const json = serializeCard(currentState.ccv3Card);
    await navigator.clipboard.writeText(json);
    showToast('已复制 JSON', 'success');
  } catch {
    showToast('复制失败', 'error');
  }
}

async function handleReset() {
  if (pageCheckInterval) {
    clearInterval(pageCheckInterval);
    pageCheckInterval = null;
  }
  
  currentState = {
    step: 0,
    pageType: 'unknown',
    theme: currentState.theme,
    basicInfo: null,
    worldbook: null,
    hiddenKeys: [],
    hiddenIndices: [],
    hiddenSettings: null,
    ccv3Card: null
  };
  
  await saveState();
  switchToStage(1);
  
  const pageType = await detectPageType();
  updatePageTypeHint(pageType);
  
  showToast('已重置', 'success');
}

function showToast(message, type = 'info') {
  const toast = elements.toast;
  const icon = elements.toastIcon;
  
  elements.toastMessage.textContent = message;
  toast.className = `toast ${type}`;
  icon.innerHTML = TOAST_ICONS[type] || TOAST_ICONS.info;
  
  toast.classList.add('show');
  
  setTimeout(() => {
    toast.classList.remove('show');
  }, 3000);
}

chrome.runtime.onMessage.addListener((message) => {
  if (message.action === 'cardDataExtracted') {
    currentState.basicInfo = message.data.basicInfo;
    currentState.hiddenKeys = message.data.hiddenKeys || [];
    currentState.hiddenIndices = Array.isArray(message.data.hiddenIndices) ? message.data.hiddenIndices : [];
    
    if (currentState.pageType === 'share') {
      showToast('基础信息已提取，请进入对话页面', 'success');
    }
    
    saveState();
  }
  
  if (message.action === 'worldbookExtracted') {
    currentState.worldbook = message.data.worldbook || [];
    if (Array.isArray(message.data.hiddenKeys) && message.data.hiddenKeys.length) {
      currentState.hiddenKeys = message.data.hiddenKeys;
    }
    if (Array.isArray(message.data.hiddenIndices) && message.data.hiddenIndices.length) {
      currentState.hiddenIndices = message.data.hiddenIndices;
    }
    
    const hasHidden = currentState.hiddenKeys.length > 0 || currentState.hiddenIndices.length > 0;
    if (hasHidden) {
      switchToStage(3);
    } else {
      buildCard();
      switchToStage(4);
    }
    
    saveState();
    showToast('世界书已提取', 'success');
  }
});

chrome.storage.onChanged.addListener((changes, areaName) => {
  if (areaName !== 'local' || !changes[STORAGE_KEY]) return;
  
  const newState = changes[STORAGE_KEY].newValue;
  if (!newState) return;
  
  currentState = { ...currentState, ...newState };
});

document.addEventListener('DOMContentLoaded', async () => {
  initElements();
  bindEvents();
  await loadState();
  
  const pageType = await detectPageType();
  currentState.pageType = pageType;
  updatePageTypeHint(pageType);
  
  if (currentState.step === 0) {
    switchToStage(1);
  } else {
    switchToStage(currentState.step);
    
    if (currentState.step === 2 && pageType === 'dream') {
      startDreamExtraction();
    }
  }
});
