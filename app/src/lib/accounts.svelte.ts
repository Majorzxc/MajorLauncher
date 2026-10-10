// Аккаунты для шапки и главного экрана.

import { api, type Account } from "./api";

export const accounts = $state({ list: [] as Account[], loaded: false });

export async function loadAccounts() {
  accounts.list = await api.accounts().catch(() => []);
  accounts.loaded = true;
}

export function setAccounts(list: Account[]) {
  accounts.list = list;
}

export function currentAccount(): Account | null {
  return accounts.list.find((a) => a.is_default) ?? accounts.list[0] ?? null;
}

export const KIND = { offline: "офлайн", ely: "Ely.by" } as const;
