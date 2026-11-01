use super::*;

fn index() -> ModIndex {
    let m = |id: &str, name: &str, file: &str, packages: &[&str]| {
        (
            vec![id.to_string()],
            ModRef {
                name: name.into(),
                file: file.into(),
                enabled: true,
                update: None,
            },
            packages.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
        )
    };
    let mut fot = m(
        "fishofthieves",
        "Fish of Thieves",
        "fishofthieves-fabric.jar",
        &["com.stevekung.fishofthieves"],
    );
    fot.1.update = Some(("v2".into(), "5.0".into()));
    ModIndex::new(vec![
        fot,
        m("iris", "Iris", "iris.jar", &["net.irisshaders.iris"]),
        m(
            "sodium",
            "Sodium",
            "sodium.jar",
            &["net.caffeinemc.mods.sodium"],
        ),
        // Одна и та же библиотека внутри двух модов — по ней мод не узнать.
        m("a", "A", "a.jar", &["com.shared.lib", "com.a.mod"]),
        m("b", "B", "b.jar", &["com.shared.lib"]),
    ])
}

fn run(text: &str, crash: bool) -> Analysis {
    let mut a = Analyzer::new(Rules::builtin());
    a.text(text);
    a.finish(crash, None, &index())
}

#[test]
fn builtin_rules_parse() {
    let r = Rules::builtin();
    assert!(r.version >= 1);
    assert!(Rules::parse(r#"{"version":1,"rules":[{"id":"x","level":"critical","any":["a"],"regex":"(","title":"t","text":"t"}]}"#).is_err());
}

/// Настоящий отчёт о падении сборки автора (24.10.2026), начало.
const AUTHOR_CRASH: &str = "---- Minecraft Crash Report ----
// I let you down. Sorry :(

Time: 2026-10-24 22:29:15
Description: Initializing game

java.lang.RuntimeException: Could not execute entrypoint stage 'main' due to errors, provided by 'fishofthieves' at 'com.stevekung.fishofthieves.fabric.FishOfThievesFabric'!
\tat net.fabricmc.loader.impl.FabricLoaderImpl.lambda$invokeEntrypoints$0(FabricLoaderImpl.java:413)
\tat net.fabricmc.loader.impl.util.ExceptionUtil.gatherExceptions(ExceptionUtil.java:33)
\tat knot//net.minecraft.client.main.Main.main(Main.java:232)
Caused by: java.lang.UnsupportedOperationException
\tat knot//com.google.common.collect.ImmutableMap.put(ImmutableMap.java:813)
\tat knot//com.stevekung.fishofthieves.registry.FOTDecoratedPotPatterns.putItemsToPotTexture(FOTDecoratedPotPatterns.java:34)
\tat knot//com.stevekung.fishofthieves.fabric.FishOfThievesFabric.onInitialize(FishOfThievesFabric.java:113)
\t... 6 more
";

#[test]
fn author_crash_names_the_mod() {
    let report = CrashReport::parse("crash.txt", AUTHOR_CRASH);
    assert_eq!(report.description.as_deref(), Some("Initializing game"));
    assert!(
        report
            .exception
            .as_deref()
            .unwrap()
            .contains("UnsupportedOperationException")
    );
    let a = Analyzer::new(Rules::builtin()).finish(true, Some(&report), &index());
    let f = &a.critical[0];
    assert_eq!(f.rule, "entrypoint");
    assert_eq!(f.title, "Мод «Fish of Thieves» упал при запуске игры");
    assert_eq!(
        f.culprit.as_ref().unwrap().file.as_deref(),
        Some("fishofthieves-fabric.jar")
    );
    // Есть обновление — сначала «Обновить», потом «Выключить».
    assert!(matches!(f.actions[0], Action::Update { .. }));
    assert!(matches!(f.actions[1], Action::Disable { .. }));
    assert_eq!(a.crash_report.as_deref(), Some("crash.txt"));
}

#[test]
fn fabric_dependency_messages() {
    let a = run(
        "[main/ERROR]: Incompatible mods found!
 - Mod 'Iris' (iris) 1.8.12 requires any version of sodium, which is missing!
 - Mod 'Iris' (iris) 1.8.12 requires version 0.6.0 or later of mod 'Sodium' (sodium), but only the wrong version is present: 0.5.11!
 - Mod 'A' (a) 1.0 is incompatible with any version of mod 'B' (b)!",
        true,
    );
    let titles: Vec<&str> = a.critical.iter().map(|f| f.title.as_str()).collect();
    assert!(
        titles.contains(&"Моду «Iris» нужен «sodium», а его нет"),
        "{titles:?}"
    );
    assert!(
        titles.contains(&"Моду «Iris» нужен другой «Sodium»"),
        "{titles:?}"
    );
    assert!(titles.contains(&"«A» несовместим с «B»"), "{titles:?}");
    let missing = a
        .critical
        .iter()
        .find(|f| f.rule == "fabric_missing")
        .unwrap();
    assert!(matches!(&missing.actions[0], Action::Search { query } if query == "sodium"));
    let breaks = a
        .critical
        .iter()
        .find(|f| f.rule == "fabric_breaks")
        .unwrap();
    assert_eq!(breaks.actions.len(), 2, "выключить любой из двух");
}

#[test]
fn many_mods_missing_one_dependency_is_one_finding() {
    // Настоящий вывод Fabric 0.17.3 (Simply Optimized без Fabric API, 01.11.2026).
    let idx = ModIndex::new(vec![(
        vec!["fabric-api".into()],
        ModRef {
            name: "Fabric API".into(),
            file: "fabric-api.jar".into(),
            enabled: false,
            update: None,
        },
        vec![],
    )]);
    let mut a = Analyzer::new(Rules::builtin());
    a.text(
        "\t - Mod 'Enhanced Block Entities' (enhancedblockentities) 0.10.2+1.21 requires any version of fabric-api, which is missing!
\t - Mod 'EntityCulling' (entityculling) 1.9.2 requires any version of fabric-api, which is missing!
\t - Mod 'No Chat Reports' (nochatreports) 1.21.1-v2.9.1 requires any version of fabric-api, which is missing!
\t - Mod 'Concurrent Chunk Management Engine (Optimizations/Natives Math)' (c2me-opts-natives-math) 0.3.0+alpha.0.358+1.21.1 requires version 22 or later of 'OpenJDK 64-Bit Server VM' (java), but only the wrong version is present: 21!",
    );
    let r = a.finish(true, None, &idx);
    let missing: Vec<&Finding> = r
        .critical
        .iter()
        .filter(|f| f.rule == "fabric_missing")
        .collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].title, "Модам нужен «fabric-api», а его нет");
    assert!(
        missing[0]
            .text
            .contains("Enhanced Block Entities, EntityCulling, No Chat Reports")
    );
    assert!(missing[0].text.contains("выключен"));
    assert_eq!(
        missing[0].actions,
        vec![Action::Enable {
            file: "fabric-api.jar".into()
        }]
    );
    // Требование Java у вложенного модуля — предупреждение: игра с ним
    // запускалась (проверено, когда включили Fabric API).
    assert_eq!(r.critical.len(), 1, "главное — нет Fabric API");
    let java = r.warnings.iter().find(|f| f.rule == "fabric_java").unwrap();
    assert!(
        java.title.contains("нужна Java 22, а игра идёт на Java 21"),
        "{}",
        java.title
    );
    assert!(
        !r.critical
            .iter()
            .chain(r.warnings.iter())
            .any(|f| f.rule == "fabric_wrong_version"),
        "частный случай раньше общего"
    );
}

#[test]
fn forge_missing_dependency() {
    let a = run(
        "Missing or unsupported mandatory dependencies:
\tMod ID: 'geckolib', Requested by: 'iris', Expected range: '[4.4,)', Actual version: '[MISSING]'",
        true,
    );
    assert_eq!(
        a.critical[0].title,
        "Моду «Iris» нужен «geckolib», а его нет"
    );
}

#[test]
fn memory_driver_and_noise() {
    let a = run(
        "Exception in thread \"main\" java.lang.OutOfMemoryError: Java heap space",
        true,
    );
    assert_eq!(a.critical[0].rule, "oom");
    assert_eq!(
        a.critical[0].actions,
        vec![Action::Memory, Action::Optimize]
    );

    let a = run(
        "[Render thread/ERROR]: GLFW error 65542: WGL: The driver does not appear to support OpenGL",
        true,
    );
    assert_eq!(a.critical[0].rule, "opengl");

    // Обычный шум модов — ничего.
    let a = run(
        "[Render thread/WARN]: Missing textures in model minecraft:item/compass_01:
[Render thread/WARN]: Unable to find banner pattern with id: 'minecraft:border'",
        false,
    );
    assert!(a.critical.is_empty() && a.warnings.is_empty(), "{a:?}");
}

#[test]
fn warnings_are_counted_and_not_critical_without_crash() {
    let a = run(
        "[Server thread/WARN]: Can't keep up! Is the server overloaded? Running 2046ms or 40 ticks behind
[Server thread/WARN]: Can't keep up! Is the server overloaded? Running 2046ms or 40 ticks behind
[Render thread/ERROR]: Couldn't load mru pack metadata: com.google.gson.stream.MalformedJsonException
[main/WARN]: @Mixin target snownee.jade.impl.BlockAccessorImpl was not found underlay.client.jade.mixins.json:BlockAccessorImplMixin from mod underlay",
        false,
    );
    assert!(a.critical.is_empty());
    let lag = a
        .warnings
        .iter()
        .find(|f| f.rule == "cant_keep_up")
        .unwrap();
    assert_eq!(lag.count, 2);
    assert!(
        a.warnings
            .iter()
            .any(|f| f.title == "Ресурспак «mru» не загрузился")
    );
    assert!(
        a.warnings
            .iter()
            .any(|f| f.title == "Мод «underlay» работает не полностью")
    );
}

#[test]
fn noise_from_real_log_is_dropped() {
    // Строки из лога автора: имя драйвера в сведениях о видеокарте — не падение;
    // пак, встроенный в мод, — не пак игрока; два одинаковых предупреждения — одно.
    let a = run(
        "[main/INFO]: Found graphics adapter: AdapterInfo{vendor=NVIDIA, openglIcdFilePath='C:\\Windows\\nvoglv64.dll'}
[Render thread/ERROR]: Couldn't load iris pack metadata: com.google.gson.stream.MalformedJsonException
[Render thread/ERROR]: Couldn't load file/Faithful 32x.zip pack metadata: Pack version
[main/WARN]: @Mixin target a.B was not found x.mixins.json:M from mod underlay
[main/WARN]: @Mixin target a.C was not found x.mixins.json:N from mod underlay",
        false,
    );
    assert!(!a.warnings.iter().any(|f| f.rule == "driver_crash"));
    let packs: Vec<&str> = a
        .warnings
        .iter()
        .filter(|f| f.rule == "pack_broken")
        .map(|f| f.title.as_str())
        .collect();
    assert_eq!(packs, vec!["Ресурспак «Faithful 32x» не загрузился"]);
    let underlay: Vec<&Finding> = a
        .warnings
        .iter()
        .filter(|f| f.rule == "mixin_target_missing")
        .collect();
    assert_eq!(underlay.len(), 1);
    assert_eq!(underlay[0].count, 2);
}

#[test]
fn unknown_crash_falls_back_to_stack() {
    // Правил под это нет — виновник по стеку: первый кадр не из игры.
    let a = run(
        "[Render thread/ERROR]: Unreported exception thrown!
java.lang.NullPointerException: Cannot invoke \"Object.toString()\" because \"x\" is null
\tat knot//net.minecraft.client.Minecraft.tick(Minecraft.java:1)
\tat knot//net.irisshaders.iris.pipeline.Thing.render(Thing.java:10)",
        true,
    );
    let f = &a.critical[0];
    assert_eq!(f.rule, "unknown");
    assert_eq!(f.title, "Игра упала — похоже, из-за мода «Iris»");
    assert!(matches!(&f.actions[0], Action::Disable { file } if file == "iris.jar"));
    // Общая библиотека двух модов на виновника не указывает.
    assert_eq!(index().by_class("com.shared.lib.X"), None);
    assert_eq!(index().by_class("com.a.mod.X"), Some("a".into()));
}

#[test]
fn errors_during_play_become_warnings_per_mod() {
    let a = run(
        "[Server thread/ERROR]: Error executing task
java.lang.NullPointerException: boom
\tat knot//net.minecraft.server.MinecraftServer.tick(MinecraftServer.java:1)
\tat knot//net.irisshaders.iris.Iris.onTick(Iris.java:5)
[Render thread/INFO]: ok",
        false,
    );
    assert!(a.critical.is_empty());
    let w = a.warnings.iter().find(|f| f.rule == "mod_error").unwrap();
    assert_eq!(w.culprit.as_ref().unwrap().id, "iris");
}

#[test]
fn forge_suspected_mods_in_report() {
    let r = CrashReport::parse(
        "c.txt",
        "Description: Ticking entity

java.lang.IllegalStateException: bad
\tat net.minecraft.world.Level.tick(Level.java:1)

Suspected Mods:
\tIris (iris), Version: 1.7
\t\tat TRANSFORMER/iris@1.7/net.irisshaders.iris.X.y(X.java:1)",
    );
    assert_eq!(r.suspects, vec!["iris".to_string()]);
    let a = Analyzer::new(Rules::builtin()).finish(true, Some(&r), &index());
    assert_eq!(a.critical[0].culprit.as_ref().unwrap().id, "iris");
}
