// Окно обрезки обложки одно на всё приложение: его открывают мастер
// создания, «Изменить» сборки и «Сделать обложкой» у скриншота.

interface Request {
  src: string;
  resolve: (png: string | null) => void;
}

export const cropper: { request: Request | null } = $state({ request: null });

/// Показать окно обрезки. Вернёт PNG в base64 (без префикса data:) или
/// `null`, если игрок передумал.
export function crop(src: string): Promise<string | null> {
  cropper.request?.resolve(null);
  return new Promise((resolve) => (cropper.request = { src, resolve }));
}

export function finish(png: string | null) {
  const r = cropper.request;
  cropper.request = null;
  r?.resolve(png);
}
