# Правила анализатора ошибок

`analyzer-rules.json` — правила, по которым MajorLauncher узнаёт ошибки игры
(раздел 5.8 ТЗ). Лаунчер скачивает этот файл с GitHub и берёт его, только если:

- подпись `analyzer-rules.json.sig` сходится с ключом, вшитым в лаунчер;
- `version` больше, чем у встроенных правил.

Иначе остаются встроенные правила (`core/src/analyzer/rules.json`).

## Как добавить правило

1. Добавьте правило в `analyzer-rules.json` и увеличьте `version`.
2. Проверьте на логе: `mlcli analyze <сборка> --log <файл>`.
3. Подпишите (ключ — только у автора, в репозиторий не кладётся):
   `mlcli rules sign --key <ключ> rules/analyzer-rules.json`
4. Закоммитьте оба файла.

Поля правила: `id`, `level` (`critical`/`warning`), `any` — подстроки для
быстрой проверки, `regex` — уточнение с именованными группами (`mod`, `dep`,
`depid`, `name`, `want`, `have` …), `title` и `text` с подстановками `{имя}`,
`actions` (`memory`, `optimize`, `driver`, `open_backups`, `disable_mod`,
`update_mod`, `disable_dep`, `find_dep`), `crash_only`, `skip_if_mod`,
`exclusive`, `group_by`.
