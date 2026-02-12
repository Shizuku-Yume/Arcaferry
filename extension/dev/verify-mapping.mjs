import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

import { mapQuackToV3 } from '../lib/ccv3.js';

function usage() {
  const cmd = path.basename(process.argv[1] || 'verify-mapping.mjs');
  console.log(`Usage: node ${cmd} <path-to-quack-json>

Expected input:
- Either a raw API wrapper: { "code": 0, "data": { ... } }
- Or the inner object directly: { ... }

This script prints key CCv3 fields so you can regression-check mapping parity.
`);
}

function asData(obj) {
  if (obj && typeof obj === 'object' && 'code' in obj && 'data' in obj) {
    return obj.data;
  }
  return obj;
}

function buildBasicInfoFromStudioCard(data) {
  const firstChar = Array.isArray(data?.charList) ? data.charList[0] : null;
  return {
    __source: 'share',
    name: firstChar?.name || data?.name || '',
    scenario: data?.scenario || '',
    personality: data?.personality || '',
    first_mes: data?.first_mes || data?.firstMes || '',
    mes_example: data?.mes_example || data?.mesExample || '',
    system_prompt: firstChar?.prompt || data?.system_prompt || data?.systemPrompt || '',
    post_history_instructions: data?.post_history_instructions || data?.postHistoryInstructions || '',
    creator: data?.creator || data?.author || '',
    author_name: data?.author || data?.authorName || '',
    creator_notes: data?.creator_notes || data?.creatorNotes || data?.intro || firstChar?.intro || '',
    greeting: data?.greeting || [],
    prologue: data?.prologue || null,
    intro: data?.intro || firstChar?.intro || '',
    tags: data?.tags || [],
    characterbooks: data?.characterbooks || [],
    customAttrs: []
  };
}

function main() {
  const inputPath = process.argv[2];
  if (!inputPath) {
    usage();
    process.exit(2);
  }

  const raw = fs.readFileSync(inputPath, 'utf8');
  const parsed = JSON.parse(raw);
  const data = asData(parsed);

  const info = buildBasicInfoFromStudioCard(data);
  const card = mapQuackToV3(info, []);

  const d = card?.data || {};
  console.log('name:', d.name);
  console.log('creator:', d.creator);
  console.log('creator_notes.len:', String(d.creator_notes || '').length);
  console.log('mes_example.len:', String(d.mes_example || '').length);
  console.log('first_mes.len:', String(d.first_mes || '').length);
  console.log('first_mes.preview:', String(d.first_mes || '').slice(0, 120).replace(/\n/g, '\\n'));
  console.log('alternate_greetings.count:', Array.isArray(d.alternate_greetings) ? d.alternate_greetings.length : 0);
  console.log('alternate_greetings[0].preview:', String(d.alternate_greetings?.[0] || '').slice(0, 120).replace(/\n/g, '\\n'));
}

main();
