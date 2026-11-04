// Meaning-based matching for Find a setting, on a static embedding model: a
// vector per token, averaged over a text and normalised. It runs entirely on
// this computer in a few microseconds, with no native runtime, so it can run
// on every keystroke. Keyword matches still come first; this ranks what the
// words alone would miss ("my scan is too quick" is Auto scan rate).
import { tokenise, type Vocabulary } from "./wordpiece";

export type StaticModel = {
  vocab: Vocabulary;
  dims: number;
  /** Row i is token i's vector. */
  vectors: Float32Array;
  unkId: number;
};

/** One text's vector: the mean of its token vectors, unit length. Null when nothing in the text is known. */
export function embed(model: StaticModel, text: string): Float32Array | null {
  const ids = tokenise(text, model.vocab).filter((id) => id >= 0 && id !== model.unkId);
  if (!ids.length) return null;
  const out = new Float32Array(model.dims);
  for (const id of ids) {
    const base = id * model.dims;
    for (let d = 0; d < model.dims; d++) out[d] += model.vectors[base + d];
  }
  let norm = 0;
  for (let d = 0; d < model.dims; d++) norm += out[d] * out[d];
  norm = Math.sqrt(norm);
  if (!norm) return null;
  for (let d = 0; d < model.dims; d++) out[d] /= norm;
  return out;
}

/** Cosine similarity of two unit vectors. */
export function similarity(a: Float32Array, b: Float32Array): number {
  let s = 0;
  for (let d = 0; d < a.length; d++) s += a[d] * b[d];
  return s;
}

/**
 * The packed model file the app ships: a little-endian header, the
 * vocabulary, then int8 vectors with one float32 scale per row.
 *
 *   u32 magic "SFM1" | u32 vocab size | u32 dims | u32 vocab bytes |
 *   vocab as UTF-8 words joined by "\n" | f32 scale[vocab] | i8 vector[vocab][dims]
 */
export function unpackModel(bytes: ArrayBuffer): StaticModel {
  const view = new DataView(bytes);
  if (view.getUint32(0, true) !== 0x314d4653) throw new Error("Not a find model file");
  const size = view.getUint32(4, true);
  const dims = view.getUint32(8, true);
  const vocabBytes = view.getUint32(12, true);
  const words = new TextDecoder().decode(new Uint8Array(bytes, 16, vocabBytes)).split("\n");
  if (words.length !== size) throw new Error("Find model vocabulary is damaged");
  const vocab: Vocabulary = new Map(words.map((w, i) => [w, i]));
  const scaleOffset = 16 + vocabBytes;
  const scales = new Float32Array(size);
  for (let i = 0; i < size; i++) scales[i] = view.getFloat32(scaleOffset + i * 4, true);
  const data = new Int8Array(bytes, scaleOffset + size * 4, size * dims);
  const vectors = new Float32Array(size * dims);
  for (let i = 0; i < size; i++) for (let d = 0; d < dims; d++) vectors[i * dims + d] = data[i * dims + d] * scales[i];
  return { vocab, dims, vectors, unkId: vocab.get("[UNK]") ?? -1 };
}

/** Packs float32 rows into the file format above, quantising each row to int8 with its own scale. */
export function packModel(words: string[], dims: number, rows: Float32Array): Uint8Array {
  const size = words.length;
  const vocabBytes = new TextEncoder().encode(words.join("\n"));
  const out = new Uint8Array(16 + vocabBytes.length + size * 4 + size * dims);
  const view = new DataView(out.buffer);
  view.setUint32(0, 0x314d4653, true);
  view.setUint32(4, size, true);
  view.setUint32(8, dims, true);
  view.setUint32(12, vocabBytes.length, true);
  out.set(vocabBytes, 16);
  const scaleOffset = 16 + vocabBytes.length;
  const dataOffset = scaleOffset + size * 4;
  for (let i = 0; i < size; i++) {
    let max = 0;
    for (let d = 0; d < dims; d++) max = Math.max(max, Math.abs(rows[i * dims + d]));
    const scale = max / 127 || 1;
    view.setFloat32(scaleOffset + i * 4, scale, true);
    for (let d = 0; d < dims; d++) out[dataOffset + i * dims + d] = Math.round(rows[i * dims + d] / scale);
  }
  return out;
}
