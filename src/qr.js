import jsQR from "jsqr";
// Local-only decoding: no image upload, camera permission or external service.
export async function readQrFile(file) {
  if (!file || file.size > 8 * 1024 * 1024)
    throw new Error("Изображение должно быть не больше 8 МиБ");
  if (!["image/png", "image/jpeg", "image/webp"].includes(file.type))
    throw new Error("Выберите PNG, JPEG или WebP");
  const bitmap = await createImageBitmap(file);
  try {
    if (bitmap.width * bitmap.height > 32000000)
      throw new Error("Изображение слишком большое. Обрежьте его до QR-кода.");
    const scale = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const result = jsQR(
      ctx.getImageData(0, 0, canvas.width, canvas.height).data,
      canvas.width,
      canvas.height,
      { inversionAttempts: "attemptBoth" },
    );
    if (!result?.data?.trim())
      throw new Error("QR-код не найден. Попробуйте более чёткое изображение.");
    if (result.data.length > 262144)
      throw new Error("Содержимое QR-кода слишком большое");
    return result.data.trim();
  } finally {
    bitmap.close();
  }
}
