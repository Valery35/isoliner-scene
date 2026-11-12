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

Над сценой плашка значков в порядке окна модуля: вид сверху, сброс,
вид по разрезу, проекция, короб, контур и обрезка, вращение, виды,
видео, кадр в буфер и в PNG, преувеличение. Тема светлая, тёмная
включается на плашке. Обрезка контуром, коридором и по отметкам
оставляет тела замкнутыми. Видео пишется кадрами PNG и склеивается
в MP4, если в PATH есть ffmpeg.

Открытый пакет перечитывается сам, когда его переписывает модуль
QGIS: кнопка «Открыть в Isoliner Scene» окна Isoliner3D (с версии
1.5.1) запускает программу и потом только обновляет пакет. Модуль
ищет программу в `%LOCALAPPDATA%\Programs\Isoliner Scene` и в PATH,
поэтому архив для Windows распаковывается в эту папку.

Та же программа работает в браузере:
https://valery35.github.io/isoliner-scene/app/. Сборка страницы -
`trunk build --release` в папке `apps/isoliner-scene`, сайт целиком
собирает `.github/workflows/pages.yml`.

## Устройство

| Крейт | Что |
|---|---|
| `scene-format` | пакет `.i3dscene`: scene.json и GLB |
| `scene-core` | камера, облёт, охват и объём тел |
| `render` | отрисовка на wgpu |
| `tool-spec` | описания инструментов из `tools.json` |
| `tools` | расчёты инструментов 2.11, 2.12, 2.14, 2.17, 2.18, 2.23, 2.24 |
| `apps/isoliner-scene` | окно |

Проверочные данные в `fixtures/` собирает модуль Isoliner3D. Правила
работы - `AGENTS.md`, план - `doc/PLAN.md`.

## Isoliner Scene (English)

A viewer for Isoliner3D scene packages (`.i3dscene`) without QGIS.
It shows the scene with the camera saved in QGIS: mine workings, ore
bodies, surfaces, lines, transparency, vertical exaggeration and
fly-through views. A toolbar follows the QGIS viewer, with a light
theme by default and a dark one on request. Clipping keeps bodies
closed, the section view shows elevation and depth scales, videos
are joined into MP4 when ffmpeg is in PATH. An open package reloads
itself when the QGIS plugin rewrites it (the «Open in Isoliner
Scene» button of Isoliner3D 1.5.1). Unpack the Windows archive into
`%LOCALAPPDATA%\Programs\Isoliner Scene` so that the plugin finds
it. The crates also bring the scene and the Isoliner3D tools to
RustGIS.

## Лицензия

Apache 2.0, файл `LICENSE`. © 2026 ООО «Информ++».
