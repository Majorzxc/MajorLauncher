// Числа и время по-русски: размеры, «3 дня назад», «36 ч».

/// Склонение: plural(5, ["день", "дня", "дней"]) → «дней».
export function plural(n: number, [one, few, many]: [string, string, string]): string {
  const m10 = n % 10;
  const m100 = n % 100;
  if (m10 === 1 && m100 !== 11) return one;
  if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return few;
  return many;
}

export function size(bytes: number): string {
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  const text = i >= 3 ? v.toFixed(1).replace(".", ",") : Math.round(v).toString();
  return `${text} ${units[i]}`;
}

/// Когда играл: «сегодня», «вчера», «3 дня назад», «2 недели назад», дата.
export function ago(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "ещё не запускалась";
  const days = Math.floor((Date.now() / 1000 - unixSeconds) / 86400);
  if (days <= 0) return "сегодня";
  if (days === 1) return "вчера";
  if (days < 7) return `${days} ${plural(days, ["день", "дня", "дней"])} назад`;
  if (days < 30) {
    const w = Math.floor(days / 7);
    return `${w} ${plural(w, ["неделю", "недели", "недель"])} назад`;
  }
  return new Date(unixSeconds * 1000).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });
}

/// Время в игре: «0 ч», «45 мин», «36 ч».
export function playtime(seconds: number): string {
  if (seconds < 3600) {
    const m = Math.floor(seconds / 60);
    return m === 0 ? "0 ч" : `${m} мин`;
  }
  return `${Math.floor(seconds / 3600)} ч`;
}

/// Через сколько дней удалится из корзины.
export function trashLeft(deletedAt: number, keepDays: number): string {
  const left = Math.max(0, keepDays - Math.floor((Date.now() / 1000 - deletedAt) / 86400));
  return left === 0 ? "удалится сегодня" : `удалится через ${left} ${plural(left, ["день", "дня", "дней"])}`;
}
