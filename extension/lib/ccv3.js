/**
 * CCv3 (Character Card V3) Format Builder
 * 
 * Ported from Arcaferry's Rust implementation (src-tauri/src/ccv3.rs)
 * Spec: chara_card_v3, version 3.0
 */

/**
 * Create a default lorebook entry
 * @param {Object} options
 * @returns {Object}
 */
export function createLorebookEntry({
  keys = [],
  secondaryKeys = [],
  content = '',
  enabled = true,
  insertionOrder = 0,
  caseSensitive = false,
  useRegex = false,
  constant = false,
  name = '',
  priority = 10,
  id = null,
  comment = '',
  selective = false,
  position = 'before_char'
} = {}) {
  return {
    keys,
    secondary_keys: secondaryKeys,
    content,
    enabled,
    insertion_order: insertionOrder,
    case_sensitive: caseSensitive,
    use_regex: useRegex,
    constant,
    name,
    priority,
    id,
    comment,
    selective,
    position
  };
}

/**
 * Create a default lorebook (character book)
 * @param {Object} options
 * @returns {Object}
 */
export function createLorebook({
  name = '',
  description = '',
  scanDepth = 50,
  tokenBudget = 500,
  recursiveScanning = false,
  entries = []
} = {}) {
  return {
    name,
    description,
    scan_depth: scanDepth,
    token_budget: tokenBudget,
    recursive_scanning: recursiveScanning,
    entries
  };
}

/**
 * Create a default character card data structure
 * @param {Object} options
 * @returns {Object}
 */
export function createCharacterCardData({
  name = '',
  description = '',
  personality = '',
  scenario = '',
  firstMes = '',
  mesExample = '',
  alternateGreetings = [],
  systemPrompt = '',
  postHistoryInstructions = '',
  characterBook = null,
  creator = '',
  creatorNotes = '',
  tags = [],
  characterVersion = '1.0',
  creationDate = null,
  modificationDate = null,
  assets = null,
  nickname = null,
  source = null,
  groupOnlyGreetings = []
} = {}) {
  // Ensure QuackAI tag is always first
  const finalTags = [...tags];
  if (!finalTags.includes('QuackAI')) {
    finalTags.unshift('QuackAI');
  }

  const now = Math.floor(Date.now() / 1000);

  return {
    name,
    description,
    personality,
    scenario,
    first_mes: firstMes,
    mes_example: mesExample,
    alternate_greetings: alternateGreetings,
    system_prompt: systemPrompt,
    post_history_instructions: postHistoryInstructions,
    character_book: characterBook,
    creator,
    creator_notes: creatorNotes,
    tags: finalTags,
    character_version: characterVersion,
    creation_date: creationDate || now,
    modification_date: modificationDate || now,
    assets,
    nickname,
    source,
    group_only_greetings: groupOnlyGreetings
  };
}

/**
 * Create a complete CCv3 character card
 * @param {Object} data - CharacterCardData object
 * @returns {Object}
 */
export function createCharacterCardV3(data) {
  return {
    spec: 'chara_card_v3',
    spec_version: '3.0',
    data: createCharacterCardData(data)
  };
}

/**
 * Format attributes as [Label: Value] blocks
 * Matches Rust format_attrs function
 * @param {Array} attrs - Array of {label, value, isVisible} objects
 * @param {boolean} visibleOnly - Only include visible attributes
 * @returns {string}
 */
export function formatAttrs(attrs, visibleOnly = true) {
  return attrs
    .filter(a => !visibleOnly || a.isVisible !== false)
    .filter(a => a.label && a.value)
    .map(a => `[${a.label}: ${a.value}]`)
    .join('\n');
}

/**
 * Format hidden attributes (isVisible = false)
 * @param {Array} attrs
 * @returns {string}
 */
export function formatHiddenAttrs(attrs) {
  return attrs
    .filter(a => a.isVisible === false)
    .filter(a => a.label && a.value)
    .map(a => `[${a.label}: ${a.value}]`)
    .join('\n');
}

/**
 * Extract personality from attributes
 * @param {Array} attrs
 * @returns {string}
 */
export function extractPersonality(attrs) {
  const key = (s) => String(s || '').trim().toLowerCase();
  const candidates = new Set(['personality', '性格', '性格特征', '性格设定']);

  const personalityAttr = attrs.find(a => candidates.has(key(a.label || a.name)));
  return personalityAttr?.value || '';
}

/**
 * Extract greetings from Quack greeting array
 * Returns [firstMes, alternateGreetings]
 * @param {Array} greetings
 * @returns {[string, string[]]}
 */
export function extractGreetings(greetings) {
  if (!Array.isArray(greetings) || greetings.length === 0) {
    return ['', []];
  }

  const values = greetings
    .map(g => g?.content || g?.text || g?.value || '')
    .filter(s => s.length > 0);

  if (values.length === 0) {
    return ['', []];
  }

  return [values[0], values.slice(1)];
}

function normalizeGreetingArray(g) {
  if (Array.isArray(g)) return g;
  // Some payloads may provide greeting as a single object/string.
  if (!g) return [];
  if (typeof g === 'string') return [{ value: g }];
  if (typeof g === 'object') return [g];
  return [];
}

function extractGreetingValues(arr) {
  if (!Array.isArray(arr)) return [];
  return arr
    .map((g) => {
      if (typeof g === 'string') return g;
      return g?.value || g?.content || g?.text || '';
    })
    .filter((s) => typeof s === 'string' && s.length > 0);
}

function getStudioPrologue(info) {
  return (
    info?.chat_info?.studio_prologue ||
    info?.chatInfo?.studioPrologue ||
    info?.chatInfo?.studio_prologue ||
    null
  );
}

/**
 * Extract greetings following Rust (Pro) rules:
 * 1) Prefer info.prologue.greetings
 * 2) Fallback to info.greeting
 * 3) Fallback to info.first_mes
 * 4) Merge chat_info.studio_prologue alternates into alternate_greetings (dedupe, != first_mes)
 */
export function extractGreetingsFromQuackInfo(info) {
  const prologueValues = extractGreetingValues(info?.prologue?.greetings);

  let firstMes = prologueValues.length ? prologueValues[0] : '';
  let alternateGreetings = prologueValues.length ? prologueValues.slice(1) : [];

  if (!firstMes) {
    const greetingArr = normalizeGreetingArray(info?.greeting);
    const [fm, alts] = extractGreetings(greetingArr);
    firstMes = fm;
    if (alternateGreetings.length === 0) {
      alternateGreetings = Array.isArray(alts) ? alts : [];
    }
  }

  if (!firstMes) {
    firstMes = info?.first_mes || info?.firstMes || '';
  }

  // Merge studio_prologue alternates (skip its first greeting).
  const studioPrologue = getStudioPrologue(info);
  const studioValues = extractGreetingValues(studioPrologue?.greetings);
  const studioAlternates = studioValues.length ? studioValues.slice(1) : [];
  if (studioAlternates.length) {
    const existing = new Set(alternateGreetings);
    for (const g of studioAlternates) {
      if (!g) continue;
      if (g === firstMes) continue;
      if (existing.has(g)) continue;
      existing.add(g);
      alternateGreetings.push(g);
    }
  }

  // Compatibility fallback: if upstream already provides alternate_greetings and we have none,
  // use it as a last resort (Rust doesn't rely on this field).
  if (alternateGreetings.length === 0 && Array.isArray(info?.alternate_greetings)) {
    alternateGreetings = info.alternate_greetings.filter(Boolean);
  }

  return [firstMes || '', alternateGreetings];
}

/**
 * Map Quack lorebook entry to CCv3 format
 * @param {Object} entry - Quack lorebook entry
 * @param {number} index
 * @returns {Object}
 */
export function mapLorebookEntry(entry, index) {
  // Parse comma-separated keys
  const parseKeys = (keysStr) => {
    if (!keysStr) return [];
    return keysStr.split(',').map(s => s.trim()).filter(s => s.length > 0);
  };

  let keys = parseKeys(entry.keys);
  const constant = entry.constant || false;

  // If keys are empty and NOT constant, fall back to name
  if (keys.length === 0 && !constant && entry.name) {
    keys = [entry.name];
  }

  const secondaryKeys = parseKeys(entry.secondary_keys || entry.secondaryKeys);

  // selective = true ONLY when secondary_keys is not empty
  const selective = secondaryKeys.length > 0;

  // Position mapping: 0 → before_char, 1 → after_char
  let position = 'before_char';
  if (entry.position === 1) {
    position = 'after_char';
  }

  return createLorebookEntry({
    keys,
    secondaryKeys,
    content: entry.content || '',
    enabled: entry.enabled !== false,
    insertionOrder: index + 1,
    caseSensitive: entry.case_sensitive || entry.caseSensitive || false,
    useRegex: entry.use_regex || entry.useRegex || false,
    constant,
    name: entry.name || '',
    priority: entry.priority || 10,
    id: index + 1,
    comment: entry.comment || '',
    selective,
    position
  });
}

/**
 * Map Quack lorebook entries to CCv3 lorebook
 * @param {Array} entries
 * @param {string} bookName
 * @returns {Object}
 */
export function mapLorebook(entries, bookName = 'Quack Lore') {
  return createLorebook({
    name: bookName,
    entries: entries.map((e, i) => mapLorebookEntry(e, i))
  });
}

/**
 * Map Quack character info to CCv3 format
 * Main mapping function
 * @param {Object} info - QuackCharacterInfo
 * @param {Array} lorebookEntries - Array of lorebook entries
 * @returns {Object}
 */
export function mapQuackToV3(info, lorebookEntries = []) {
  // Collect all attrs
  const allAttrs = [];
  
  const extractValue = (a) => a?.value || a?.content || a?.text || a?.desc || a?.description || '';
  const pushAttrs = (list) => {
    if (!Array.isArray(list)) return;
    for (const a of list) {
      const label = a?.label || a?.name || a?.key || '';
      const value = extractValue(a);
      if (!label && !value) continue;
      allAttrs.push({ label, value, isVisible: a?.isVisible !== false });
    }
  };

  pushAttrs(info.customAttrs);
  const firstChar = info?.charList?.[0];
  pushAttrs(firstChar?.attrs);
  pushAttrs(firstChar?.adviseAttrs);
  pushAttrs(firstChar?.customAttrs);

  // Build description from visible attrs
  const description = formatAttrs(allAttrs, true);

  // Extract personality
  const personality = info.personality || extractPersonality(allAttrs);

  const [firstMes, alternateGreetings] = extractGreetingsFromQuackInfo(info);

  // Build system prompt with hidden attrs
  // Match Pro (Rust): prefer charList[0].prompt over info.system_prompt.
  let systemPrompt = firstChar?.prompt || info.system_prompt || info.systemPrompt || '';
  const hiddenAttrsBlock = formatHiddenAttrs(allAttrs);
  if (hiddenAttrsBlock) {
    systemPrompt = systemPrompt 
      ? `${systemPrompt}\n\n${hiddenAttrsBlock}`
      : hiddenAttrsBlock;
  }

  // Build lorebook
  let characterBook = null;
  if (lorebookEntries.length > 0) {
    const bookName = (firstChar?.name || info.name || '') ? `${firstChar?.name || info.name}的世界书` : 'Quack Lore';
    characterBook = mapLorebook(lorebookEntries, bookName);
  } else if (info.characterbooks?.length > 0) {
    const allEntries = info.characterbooks
      .flatMap(b => b.entryList || b.entry_list || []);
    if (allEntries.length > 0) {
      const bookName = (firstChar?.name || info.name || '') ? `${firstChar?.name || info.name}的世界书` : 'Quack Lore';
      characterBook = mapLorebook(allEntries, bookName);
    }
  }

  // Extract tags
  const tags = Array.isArray(info.tags) ? [...info.tags] : [];

  // Match Pro (Rust): name prefers charList[0].name.
  const name = firstChar?.name || info.name || '';

  // Match Pro (Rust): mes_example prefers chat_info.char_mes_example.
  const chatMesExample =
    info?.chat_info?.char_mes_example ||
    info?.chatInfo?.charMesExample ||
    info?.chatInfo?.char_mes_example ||
    '';

  // Match Pro (Rust): creator prefers author_name.
  const creator =
    info?.author_name ||
    info?.authorName ||
    info?.creator ||
    '';

  // Match Pro (Rust): creator_notes prefers chat_info.char_creator_notes, then creator_notes, then intro.
  const chatCreatorNotes =
    info?.chat_info?.char_creator_notes ||
    info?.chatInfo?.charCreatorNotes ||
    info?.chatInfo?.char_creator_notes ||
    '';
  const creatorNotes =
    chatCreatorNotes ||
    info?.creator_notes ||
    info?.creatorNotes ||
    info?.intro ||
    '';

  return createCharacterCardV3({
    name,
    description,
    personality,
    scenario: info.scenario || '',
    firstMes,
    mesExample: chatMesExample || info.mes_example || info.mesExample || '',
    alternateGreetings,
    systemPrompt,
    postHistoryInstructions: info.post_history_instructions || info.postHistoryInstructions || '',
    characterBook,
    creator,
    creatorNotes,
    tags,
    characterVersion: '1.0'
  });
}

/**
 * Serialize CCv3 card to JSON string
 * @param {Object} card
 * @returns {string}
 */
export function serializeCard(card) {
  return JSON.stringify(card, null, 2);
}

/**
 * Map CCv3 card to V2 (Tavern) format
 * @param {Object} v3Card
 * @returns {Object}
 */
export function mapV3ToV2(v3Card) {
  const d = v3Card.data || {};
  return {
    name: d.name || '',
    description: d.description || '',
    personality: d.personality || '',
    scenario: d.scenario || '',
    first_mes: d.first_mes || '',
    mes_example: d.mes_example || '',
    creator_notes: d.creator_notes || '',
    system_prompt: d.system_prompt || '',
    post_history_instructions: d.post_history_instructions || '',
    alternate_greetings: d.alternate_greetings || [],
    character_book: d.character_book,
    tags: d.tags || [],
    creator: d.creator || '',
    character_version: d.character_version || '',
    extensions: {}
  };
}

/**
 * Parse CCv3 JSON string to object
 * @param {string} json
 * @returns {Object}
 */
export function parseCard(json) {
  const card = JSON.parse(json);
  if (card.spec !== 'chara_card_v3') {
    throw new Error(`Invalid card spec: ${card.spec}`);
  }
  return card;
}

export default {
  createCharacterCardV3,
  createCharacterCardData,
  createLorebook,
  createLorebookEntry,
  formatAttrs,
  formatHiddenAttrs,
  extractGreetings,
  extractPersonality,
  mapQuackToV3,
  mapLorebook,
  mapLorebookEntry,
  serializeCard,
  parseCard
};
