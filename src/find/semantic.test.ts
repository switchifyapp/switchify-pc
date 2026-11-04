import { describe, expect, it } from "vitest";
import { embed, packModel, unpackModel, similarity, type StaticModel } from "./semantic";
import { pretokenise, tokenise } from "./wordpiece";
const words = ["[UNK]", "scanning", "speed", "un", "##believ", "##able", ",", "!"];
const vocab = new Map(words.map((w, i) => [w, i]));
describe("static embeddings", () => {
  it("splits words greedily without inserting special tokens", () => {
    const pieces = (text: string) => tokenise(text, vocab).map((i) => words[i]);
    expect(pieces("scanning speed")).toEqual(["scanning", "speed"]);
    expect(pieces("unbelievable")).toEqual(["un", "##believ", "##able"]);
    expect(pieces("Scanning, SPEED!")).toEqual(["scanning", ",", "speed", "!"]);
    expect(pieces("unbelievably")).toEqual(["[UNK]"]);
    expect(pieces("a".repeat(101))).toEqual(["[UNK]"]);
    expect(pretokenise("Café speed")).toEqual(["cafe", "speed"]);
  });
  it("averages known tokens and normalises, dropping unknown tokens", () => {
    const model: StaticModel = { vocab: new Map([["[UNK]", 0], ["one", 1], ["two", 2]]), dims: 2, vectors: new Float32Array([99, 99, 2, 0, 0, 2]), unkId: 0 };
    const v = embed(model, "one two unknown")!;
    expect(v[0]).toBeCloseTo(Math.SQRT1_2);
    expect(v[1]).toBeCloseTo(Math.SQRT1_2);
    expect(similarity(v, v)).toBeCloseTo(1);
    expect(embed(model, "unknown")).toBeNull();
    expect(embed(model, "")).toBeNull();
  });
  it("round trips per-row quantisation including zero rows and UTF-8 words", () => {
    const rows = new Float32Array([0, 0, -0.25, 1, 0.4, -0.7]);
    const model = unpackModel(packModel(["[UNK]", "one", "café"], 2, rows).buffer as ArrayBuffer);
    expect([...model.vocab.keys()]).toEqual(["[UNK]", "one", "café"]);
    for (let i = 0; i < rows.length; i++) expect(Math.abs(model.vectors[i] - rows[i])).toBeLessThan(0.005);
    expect(() => unpackModel(new ArrayBuffer(4))).toThrow();
    expect(() => unpackModel(new ArrayBuffer(16))).toThrow();
    expect(() => unpackModel(packModel(["one"], 2, new Float32Array([1, 2])).slice(0, -1).buffer as ArrayBuffer)).toThrow();
  });
});
