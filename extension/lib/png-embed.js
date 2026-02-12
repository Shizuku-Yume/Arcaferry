/**
 * PNG Metadata Embedding Library
 * 
 * Ported from Arcaferry's Rust implementation (src-tauri/src/png.rs)
 * Handles tEXt chunk insertion for CCv3 character cards
 * 
 * Key constraints:
 * - IDAT chunks must be preserved exactly (no re-encoding)
 * - tEXt chunk keyword: "ccv3"
 * - Value: Base64 encoded JSON
 * - Insertion position: before IEND
 */

const PNG_SIGNATURE = new Uint8Array([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

// ============================================================================
// CRC32 Implementation (matching PNG spec)
// ============================================================================

function createCRC32Table() {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    }
    table[n] = c;
  }
  return table;
}

const CRC32_TABLE = createCRC32Table();

/**
 * Calculate CRC32 for a buffer
 * @param {Uint8Array} buf
 * @param {number} seed
 * @returns {number}
 */
function crc32(buf, seed = 0) {
  let crc = seed ^ -1;
  for (let i = 0; i < buf.length; i++) {
    crc = (crc >>> 8) ^ CRC32_TABLE[(crc ^ buf[i]) & 0xFF];
  }
  return (crc ^ -1) >>> 0; // Convert to unsigned
}

/**
 * Calculate CRC32 for chunk (type + data)
 * @param {Uint8Array} chunkType - 4 bytes
 * @param {Uint8Array} data
 * @returns {number}
 */
function calculateChunkCRC(chunkType, data) {
  const combined = new Uint8Array(chunkType.length + data.length);
  combined.set(chunkType, 0);
  combined.set(data, chunkType.length);
  return crc32(combined);
}

// ============================================================================
// PNG Chunk Reading/Writing
// ============================================================================

/**
 * @typedef {Object} PngChunk
 * @property {string} type - 4-character chunk type
 * @property {Uint8Array} data - Chunk data
 */

/**
 * Read all chunks from PNG data
 * @param {Uint8Array} data
 * @returns {PngChunk[]}
 */
export function readChunks(data) {
  // Verify PNG signature
  for (let i = 0; i < 8; i++) {
    if (data[i] !== PNG_SIGNATURE[i]) {
      throw new Error('Invalid PNG signature');
    }
  }

  const chunks = [];
  let pos = 8;

  while (pos < data.length) {
    if (pos + 8 > data.length) break;

    // Read length (big-endian)
    const length = (data[pos] << 24) | (data[pos + 1] << 16) | 
                   (data[pos + 2] << 8) | data[pos + 3];
    pos += 4;

    // Read chunk type
    const typeBytes = data.slice(pos, pos + 4);
    const type = String.fromCharCode(...typeBytes);
    pos += 4;

    if (pos + length + 4 > data.length) {
      throw new Error('Truncated PNG chunk');
    }

    // Read chunk data
    const chunkData = data.slice(pos, pos + length);
    pos += length;

    // Skip CRC (we recalculate on write)
    pos += 4;

    chunks.push({ type, data: chunkData });

    // Stop at IEND
    if (type === 'IEND') break;
  }

  return chunks;
}

/**
 * Build PNG from chunks
 * @param {PngChunk[]} chunks
 * @returns {Uint8Array}
 */
export function buildPng(chunks) {
  // Calculate total size
  let totalSize = 8; // PNG signature
  for (const chunk of chunks) {
    totalSize += 4 + 4 + chunk.data.length + 4; // length + type + data + crc
  }

  const result = new Uint8Array(totalSize);
  let pos = 0;

  // Write PNG signature
  result.set(PNG_SIGNATURE, pos);
  pos += 8;

  // Write each chunk
  for (const chunk of chunks) {
    const length = chunk.data.length;
    const typeBytes = new Uint8Array([
      chunk.type.charCodeAt(0),
      chunk.type.charCodeAt(1),
      chunk.type.charCodeAt(2),
      chunk.type.charCodeAt(3)
    ]);

    // Write length (big-endian)
    result[pos++] = (length >>> 24) & 0xFF;
    result[pos++] = (length >>> 16) & 0xFF;
    result[pos++] = (length >>> 8) & 0xFF;
    result[pos++] = length & 0xFF;

    // Write type
    result.set(typeBytes, pos);
    pos += 4;

    // Write data
    result.set(chunk.data, pos);
    pos += chunk.data.length;

    // Calculate and write CRC
    const crcValue = calculateChunkCRC(typeBytes, chunk.data);
    result[pos++] = (crcValue >>> 24) & 0xFF;
    result[pos++] = (crcValue >>> 16) & 0xFF;
    result[pos++] = (crcValue >>> 8) & 0xFF;
    result[pos++] = crcValue & 0xFF;
  }

  return result;
}

// ============================================================================
// tEXt Chunk Handling
// ============================================================================

/**
 * Build tEXt chunk data: keyword + null + base64(text)
 * @param {string} keyword
 * @param {string} text
 * @returns {Uint8Array}
 */
function buildTextChunkData(keyword, text) {
  // Base64 encode the text
  const encoded = btoa(unescape(encodeURIComponent(text)));
  
  const keywordBytes = new TextEncoder().encode(keyword);
  const encodedBytes = new TextEncoder().encode(encoded);
  
  const data = new Uint8Array(keywordBytes.length + 1 + encodedBytes.length);
  data.set(keywordBytes, 0);
  data[keywordBytes.length] = 0; // Null separator
  data.set(encodedBytes, keywordBytes.length + 1);
  
  return data;
}

/**
 * Decode tEXt chunk data
 * @param {Uint8Array} data
 * @returns {[string, string] | null} [keyword, text]
 */
export function decodeTextChunk(data) {
  const nullPos = data.indexOf(0);
  if (nullPos === -1) return null;

  const keyword = new TextDecoder().decode(data.slice(0, nullPos));
  const textData = data.slice(nullPos + 1);
  const textStr = new TextDecoder().decode(textData);

  // Try Base64 decode
  try {
    const decoded = decodeURIComponent(escape(atob(textStr)));
    return [keyword, decoded];
  } catch {
    // Fall back to raw text
    return [keyword, textStr];
  }
}

/**
 * Read all text chunks from PNG
 * @param {Uint8Array} data
 * @returns {Map<string, string>}
 */
export function readTextChunks(data) {
  const chunks = readChunks(data);
  const result = new Map();

  for (const chunk of chunks) {
    if (chunk.type === 'tEXt') {
      const decoded = decodeTextChunk(chunk.data);
      if (decoded) {
        result.set(decoded[0], decoded[1]);
      }
    }
  }

  return result;
}

/**
 * Inject or replace a tEXt chunk in PNG
 * @param {Uint8Array} data - Original PNG data
 * @param {string} keyword
 * @param {string} text
 * @param {boolean} replace - Replace existing chunk with same keyword
 * @returns {Uint8Array}
 */
export function injectTextChunk(data, keyword, text, replace = true) {
  const chunks = readChunks(data);
  const newChunkData = buildTextChunkData(keyword, text);
  const newChunk = { type: 'tEXt', data: newChunkData };

  const newChunks = [];
  let replaced = false;

  for (const chunk of chunks) {
    // Check if this is a text chunk we should replace
    if (replace && chunk.type === 'tEXt') {
      const decoded = decodeTextChunk(chunk.data);
      if (decoded && decoded[0] === keyword) {
        newChunks.push(newChunk);
        replaced = true;
        continue;
      }
    }
    newChunks.push(chunk);
  }

  // If not replaced, insert before IEND
  if (!replaced) {
    const iendIndex = newChunks.findIndex(c => c.type === 'IEND');
    if (iendIndex !== -1) {
      newChunks.splice(iendIndex, 0, newChunk);
    } else {
      newChunks.push(newChunk);
    }
  }

  return buildPng(newChunks);
}

/**
 * Remove text chunk with specified keyword
 * @param {Uint8Array} data
 * @param {string} keyword
 * @returns {Uint8Array}
 */
export function removeTextChunk(data, keyword) {
  const chunks = readChunks(data);
  const newChunks = chunks.filter(chunk => {
    if (chunk.type === 'tEXt') {
      const decoded = decodeTextChunk(chunk.data);
      return !(decoded && decoded[0] === keyword);
    }
    return true;
  });
  return buildPng(newChunks);
}

/**
 * Get card data from PNG (prefers ccv3 over chara)
 * @param {Uint8Array} data
 * @returns {[string, string] | null} [format, json]
 */
export function getCardData(data) {
  const textChunks = readTextChunks(data);
  
  if (textChunks.has('ccv3')) {
    return ['ccv3', textChunks.get('ccv3')];
  }
  
  if (textChunks.has('chara')) {
    return ['chara', textChunks.get('chara')];
  }
  
  return null;
}

import { mapV3ToV2 } from './ccv3.js';

/**
 * Embed CCv3 card data into PNG
 * @param {Uint8Array} pngData
 * @param {string} cardJson
 * @returns {Uint8Array}
 */
export function embedCard(pngData, cardJson) {
  let data = injectTextChunk(pngData, 'ccv3', cardJson, true);
  
  try {
    const v3Card = JSON.parse(cardJson);
    const v2Card = mapV3ToV2(v3Card);
    const v2Json = JSON.stringify(v2Card);
    data = injectTextChunk(data, 'chara', v2Json, true);
  } catch (e) {
    console.warn('Failed to embed V2 chara chunk:', e);
  }
  
  return data;
}

/**
 * Extract IDAT chunks for integrity verification
 * @param {Uint8Array} data
 * @returns {Uint8Array[]}
 */
export function extractIdatChunks(data) {
  const chunks = readChunks(data);
  return chunks
    .filter(c => c.type === 'IDAT')
    .map(c => c.data);
}

// ============================================================================
// High-Level API for Browser
// ============================================================================

/**
 * Load image from URL and return as Uint8Array
 * @param {string} url
 * @returns {Promise<Uint8Array>}
 */
export async function fetchImageAsBytes(url) {
  const response = await fetch(url);
  const blob = await response.blob();
  const buffer = await blob.arrayBuffer();
  return new Uint8Array(buffer);
}

/**
 * Convert Blob to Uint8Array
 * @param {Blob} blob
 * @returns {Promise<Uint8Array>}
 */
export async function blobToBytes(blob) {
  const buffer = await blob.arrayBuffer();
  return new Uint8Array(buffer);
}

/**
 * Convert Uint8Array to Blob
 * @param {Uint8Array} bytes
 * @param {string} type
 * @returns {Blob}
 */
export function bytesToBlob(bytes, type = 'image/png') {
  return new Blob([bytes], { type });
}

/**
 * Create downloadable PNG with embedded card data
 * @param {string} imageUrl - Avatar URL
 * @param {Object} card - CCv3 card object
 * @returns {Promise<Blob>}
 */
export async function createCardPng(imageUrl, card) {
  const imageBytes = await fetchImageAsBytes(imageUrl);
  const cardJson = JSON.stringify(card);
  const embeddedBytes = embedCard(imageBytes, cardJson);
  return bytesToBlob(embeddedBytes);
}

/**
 * Download a blob as file
 * @param {Blob} blob
 * @param {string} filename
 */
export function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export default {
  readChunks,
  buildPng,
  readTextChunks,
  decodeTextChunk,
  injectTextChunk,
  removeTextChunk,
  getCardData,
  embedCard,
  extractIdatChunks,
  fetchImageAsBytes,
  blobToBytes,
  bytesToBlob,
  createCardPng,
  downloadBlob
};
