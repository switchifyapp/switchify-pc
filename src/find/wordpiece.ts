// A BERT WordPiece tokenizer, enough to drive a static embedding model:
// lower-case, strip accents, split on whitespace and punctuation, then
// greedily match the longest known piece. Pure TypeScript, no dependencies,
// so it runs the same in the app, in tests and in the build-time scripts.

export type Vocabulary = Map<string, number>;

const isPunctuation = (ch: string) => /[\p{P}\p{S}]/u.test(ch);

/** The words and punctuation marks of a text, lower-cased with accents removed. */
export function pretokenise(text: string): string[] {
  const clean = text.normalize("NFD").replace(/\p{Mn}/gu, "").toLowerCase();
  const out: string[] = [];
  let word = "";
  for (const ch of clean) {
    if (/\s/.test(ch) || ch === "\u0000" || ch === "�") { if (word) out.push(word); word = ""; continue; }
    if (isPunctuation(ch)) { if (word) out.push(word); word = ""; out.push(ch); continue; }
    word += ch;
  }
  if (word) out.push(word);
  return out;
}

/** Token ids for a text. Unknown words become `unk`, and a word longer than 100 characters is unknown. */
export function tokenise(text: string, vocab: Vocabulary, unk = "[UNK]"): number[] {
  const unkId = vocab.get(unk) ?? -1;
  const ids: number[] = [];
  for (const word of pretokenise(text)) {
    if (word.length > 100) { ids.push(unkId); continue; }
    const pieces: number[] = [];
    let start = 0;
    let bad = false;
    while (start < word.length) {
      let end = word.length;
      let found = -1;
      while (start < end) {
        const piece = (start > 0 ? "##" : "") + word.slice(start, end);
        const id = vocab.get(piece);
        if (id !== undefined) { found = id; break; }
        end--;
      }
      if (found < 0) { bad = true; break; }
      pieces.push(found);
      start = end;
    }
    if (bad) ids.push(unkId); else ids.push(...pieces);
  }
  return ids;
}

/** Reads the vocabulary out of a Hugging Face tokenizer.json. */
export function vocabularyFromTokenizerJson(json: { model: { vocab: Record<string, number> } }): Vocabulary {
  return new Map(Object.entries(json.model.vocab));
}
