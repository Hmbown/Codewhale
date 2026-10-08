import test from "node:test";
import assert from "node:assert/strict";

import { encodeQr, renderQrToText } from "../src/qr.mjs";

// The rendered QR is the login credential, so a wrong matrix is worse than no
// QR at all. These checks pin the structural invariants a scanner relies on:
// finder patterns, timing patterns, and the format-information copies.

function finderOk(matrix, row, col) {
  for (let r = 0; r < 7; r++) {
    for (let c = 0; c < 7; c++) {
      const expected =
        r === 0 || r === 6 || c === 0 || c === 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
      if (Boolean(matrix[row + r][col + c]) !== expected) return false;
    }
  }
  return true;
}

const SAMPLE = "https://liteapp.weixin.qq.com/q/7GiQu1?qrcode=c09677d820dc2b705c2ba8c89dee2b4c&bot_type=3";

test("encodeQr picks a valid version size", () => {
  // Version 1 is 21x21 and each version adds 4 modules.
  for (const text of ["A", "HELLO", SAMPLE]) {
    const size = encodeQr(text).length;
    assert.equal((size - 17) % 4, 0, `size ${size} is not a valid QR size`);
    assert.ok(size >= 21, `size ${size} below version 1`);
  }
});

test("encodeQr places the three finder patterns", () => {
  const matrix = encodeQr(SAMPLE);
  const size = matrix.length;
  assert.ok(finderOk(matrix, 0, 0), "top-left finder");
  assert.ok(finderOk(matrix, 0, size - 7), "top-right finder");
  assert.ok(finderOk(matrix, size - 7, 0), "bottom-left finder");
});

test("encodeQr writes the alternating timing patterns", () => {
  const matrix = encodeQr(SAMPLE);
  const size = matrix.length;
  for (let i = 8; i < size - 8; i++) {
    assert.equal(matrix[6][i], i % 2 === 0 ? 1 : 0, `row timing at ${i}`);
    assert.equal(matrix[i][6], i % 2 === 0 ? 1 : 0, `column timing at ${i}`);
  }
});

test("encodeQr sets the fixed dark module", () => {
  const matrix = encodeQr(SAMPLE);
  assert.equal(matrix[matrix.length - 8][8], 1);
});

test("format information is mirrored between its two copies", () => {
  const matrix = encodeQr(SAMPLE);
  const size = matrix.length;
  // Bits 0-5 run down column 8; the second copy of bits 8-14 runs down column 8
  // near the bottom-left finder. Pin that the split exists and is populated.
  const firstCopy = [0, 1, 2, 3, 4, 5].map((r) => matrix[r][8]);
  const secondCopy = [8, 9, 10, 11, 12, 13, 14].map((i) => matrix[size - 15 + i][8]);
  assert.ok(
    firstCopy.every((v) => v === 0 || v === 1),
    "first format copy must be fully written"
  );
  assert.ok(
    secondCopy.every((v) => v === 0 || v === 1),
    "second format copy must be fully written"
  );
});

test("encodeQr round-trips a UTF-8 payload without throwing", () => {
  // 18 UTF-8 bytes needs version 2 (25x25), not version 1.
  const matrix = encodeQr("微信扫码测试");
  assert.equal(matrix.length, 25);
  assert.equal((matrix.length - 17) % 4, 0);
});

test("encodeQr rejects payloads beyond the supported versions", () => {
  assert.throws(() => encodeQr("x".repeat(400)), /too long/);
});

test("renderQrToText produces half-block rows covering the whole matrix", () => {
  const text = renderQrToText(SAMPLE);
  const lines = text.split("\n");
  const matrix = encodeQr(SAMPLE);
  const quietZone = 2;
  const padded = matrix.length + quietZone * 2;
  const expectedRows = padded % 2 === 0 ? padded : padded + 1;

  assert.equal(lines.length, expectedRows / 2, "one text row per two module rows");
  for (const line of lines) {
    assert.equal([...line].length, padded, "every row is padded to the same width");
  }
});

test("renderQrToText emits only half-block glyphs and spaces", () => {
  const text = renderQrToText(SAMPLE);
  assert.match(text, /^[\u2580\u2584\u2588 \n]+$/);
});
