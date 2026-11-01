// Раздел «Скины» (раздел 5.9 ТЗ): библиотека, выбранный скин (его показывает
// 3D-просмотр) и настройки. Выбран может быть скин из библиотеки, из
// каталога, найденный по нику или скин аккаунта Ely.by.

import { api, type CatalogSkin, type NickSource, type Skin, type SkinCatalogId, type SkinSettings, type Textures } from "./api";
import { decode, type Decoded } from "./skinrender";

export type Selected =
  | { kind: "lib"; skin: Skin }
  | { kind: "cat"; item: CatalogSkin; catalog: SkinCatalogId }
  | { kind: "nick"; textures: Textures; from: NickSource }
  | { kind: "account"; textures: Textures };

export const skins = $state({
  view: "mine" as "mine" | "catalog",
  list: [] as Skin[],
  loaded: false,
  selected: null as Selected | null,
  settings: { editor: null, show_sensitive: false } as SkinSettings,
  /// Меняется, когда файл скина изменили в редакторе: 3D и фигуры перерисуются.
  version: 0,
  /// Скин, надетый на текущий офлайн-аккаунт (id из библиотеки).
  worn: null as string | null,
  /// Плащ текущего офлайн-аккаунта (пиксели для 3D).
  cape: null as ImageData | null,
});

/// CustomSkinLoader на Modrinth: показывает офлайн-нику его скин в игре.
export const SKIN_LOADER = "idMHQ4n2";

export async function loadWorn(account: string | null) {
  skins.worn = account ? await api.skinWorn(account).catch(() => null) : null;
  await loadCape(account);
}

export async function loadCape(account: string | null) {
  const bytes = account ? await api.skinCape(account).catch(() => null) : null;
  skins.cape = bytes && bytes.byteLength ? await decode(bytes).then((d) => d.image).catch(() => null) : null;
}

export async function wear(account: string, skin: string | null) {
  await api.skinWear(account, skin);
  skins.worn = skin;
}

export async function loadSkins() {
  skins.list = await api.skins().catch(() => []);
  skins.loaded = true;
  api
    .config()
    .then((c) => {
      if (c.skins) skins.settings = c.skins;
    })
    .catch(() => {});
}

export async function saveSkinSettings(patch: Partial<SkinSettings>) {
  const next = { ...skins.settings, ...patch };
  skins.settings = next;
  const c = await api.setSkins(next);
  if (c.skins) skins.settings = c.skins;
}

/// Ключ и загрузчик картинки выбранного скина.
export function source(sel: Selected): { key: string; load: () => Promise<ArrayBuffer> } {
  switch (sel.kind) {
    case "lib":
      return { key: sel.skin.id, load: () => api.skinPng(sel.skin.id) };
    case "cat":
      return { key: sel.item.url, load: () => api.skinTexture(sel.item.url) };
    default:
      return { key: sel.textures.skin!, load: () => api.skinTexture(sel.textures.skin!) };
  }
}

/// Пиксели выбранного скина и плаща для 3D.
export async function pixels(sel: Selected): Promise<{ skin: Decoded; cape: ImageData | null }> {
  const skin = await decode(await source(sel).load());
  let cape: ImageData | null = null;
  if ((sel.kind === "nick" || sel.kind === "account") && sel.textures.cape) {
    cape = await api
      .skinTexture(sel.textures.cape)
      .then(decode)
      .then((d) => d.image)
      .catch(() => null);
  }
  return { skin, cape };
}

/// Тонкие ли руки: что известно о скине; `null` — по картинке.
export function knownSlim(sel: Selected): boolean | null {
  switch (sel.kind) {
    case "lib":
      return sel.skin.slim;
    case "cat":
      return sel.item.slim;
    default:
      return sel.textures.slim;
  }
}
