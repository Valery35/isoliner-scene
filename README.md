# Isoliner Scene

Сцена модуля QGIS [Isoliner3D](https://github.com/Valery35/qgis-isoliner3d)
без QGIS. Программа открывает пакет сцены `.i3dscene`, который пишет
окно просмотра Isoliner3D, и показывает его той же камерой: тела
выработок и залежей, поверхности, линии, прозрачность, преувеличение
по вертикали, виды облёта.

Крейты программы - сцена и инструменты Isoliner3D на Rust - подключаются
к RustGIS.

## Запуск

```
cargo run -p isoliner-scene --release -- fixtures/scene_demo.i3dscene
```

Левая кнопка мыши вращает сцену, правая сдвигает, колесо приближает.
Двойной щелчок вписывает сцену в окно.

## Устройство

| Крейт | Что |
|---|---|
| `scene-format` | пакет `.i3dscene`: scene.json и GLB |
| `scene-core` | камера, облёт, охват и объём тел |
| `render` | отрисовка на wgpu |
| `tool-spec` | описания инструментов из `tools.json` |
| `tools` | расчёты инструментов 2.23 и 2.24 |
| `apps/isoliner-scene` | окно |

Проверочные данные в `fixtures/` собирает модуль Isoliner3D. Правила
работы - `AGENTS.md`, план - `doc/PLAN.md`.

## Isoliner Scene (English)

A viewer for Isoliner3D scene packages (`.i3dscene`) without QGIS.
It shows the scene with the camera saved in QGIS: mine workings, ore
bodies, surfaces, lines, transparency, vertical exaggeration and
fly-through views. The crates also bring the scene and the Isoliner3D
tools to RustGIS.

## Лицензия

Apache 2.0, файл `LICENSE`. © 2026 ООО «Информ++».
