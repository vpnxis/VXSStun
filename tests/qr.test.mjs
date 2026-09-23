import test from "node:test";
import assert from "node:assert/strict";
import QRCode from "qrcode";
import { readQrFile } from "../src/qr.js";

test("QR reader rejects unsupported types and oversized input before decoding", async () => {
  await assert.rejects(
    readQrFile({ size: 9 * 1024 * 1024, type: "image/png" }),
    /8 МиБ/,
  );
  await assert.rejects(readQrFile({ size: 20, type: "image/svg+xml" }), /PNG/);
});
test("QR reader decodes a generated image locally and releases its bitmap", async (t) => {
  const content =
    "vless://00000000-0000-4000-8000-000000000001@example.invalid:443?security=tls#QR-test";
  const matrix = QRCode.create(content).modules;
  const scale = 5,
    border = 4,
    size = (matrix.size + border * 2) * scale;
  const pixels = new Uint8ClampedArray(size * size * 4).fill(255);
  for (let y = 0; y < size; y++)
    for (let x = 0; x < size; x++) {
      const mx = Math.floor(x / scale) - border,
        my = Math.floor(y / scale) - border;
      if (
        mx >= 0 &&
        my >= 0 &&
        mx < matrix.size &&
        my < matrix.size &&
        matrix.get(my, mx)
      )
        pixels.set([0, 0, 0, 255], (y * size + x) * 4);
    }
  let closed = false;
  const oldBitmap = globalThis.createImageBitmap,
    oldDocument = globalThis.document;
  t.after(() => {
    globalThis.createImageBitmap = oldBitmap;
    globalThis.document = oldDocument;
  });
  globalThis.createImageBitmap = async () => ({
    width: size,
    height: size,
    close() {
      closed = true;
    },
  });
  globalThis.document = {
    createElement: () => ({
      getContext: () => ({
        fillRect() {},
        drawImage() {},
        getImageData: () => ({ data: pixels }),
      }),
    }),
  };
  assert.equal(await readQrFile({ size: 1024, type: "image/png" }), content);
  assert.ok(closed);
  closed = false;
  pixels.fill(255);
  await assert.rejects(
    readQrFile({ size: 1024, type: "image/png" }),
    /не найден/,
  );
  assert.ok(closed);
});
