// Проблемы последней игры каждой сборки (анализатор ошибок) — для значка на
// плитке. Перечитывается после каждой игры.

import { api } from "./api";

export const problems = $state({ counts: {} as Record<string, number> });

export async function loadProblems() {
  problems.counts = await api.problemsCounts().catch(() => ({}));
}
