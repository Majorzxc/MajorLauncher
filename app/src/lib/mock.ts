// Заглушки команд ядра для проверки вёрстки в браузере (только `npm run dev`).

const KEY = "mock-config";

// Параметры адреса для снимков: ?mode=light&accent=%237c3aed&bg=%23202a44
const url = new URLSearchParams(location.search);

function config() {
  const saved = localStorage.getItem(KEY);
  if (saved) return JSON.parse(saved);
  return {
    data_dir: null,
    appearance: {
      mode: url.get("mode") ?? "system",
      accent: url.get("accent"),
      background: url.get("bg"),
    },
  };
}

export async function mock<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case "core_version":
      return "0.1.0 (браузер)" as T;
    case "get_config":
      return config() as T;
    case "set_appearance": {
      const c = { ...config(), appearance: args?.appearance };
      localStorage.setItem(KEY, JSON.stringify(c));
      return c as T;
    }
    case "current_account":
      return { name: "Majorzxc", kind: "Ely.by" } as T;
    default:
      throw new Error(`нет заглушки для команды ${cmd}`);
  }
}
