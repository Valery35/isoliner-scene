# Продвижение Isoliner3D - черновики, ноябрь 2026

Подготовлено 15 ноября 2026 года по слову автора «Думаю, Isoliner3D стоит
продвинуть». Не опубликовано. Площадки группы «иногда» и витрины
публикуются только по слову автора в чате основной сессии Isoliner3D.

## Что есть на показ

| Приложение | Версия | Где |
|---|---|---|
| Модуль QGIS Isoliner3D | 1.5.7, 35 инструментов Processing | https://plugins.qgis.org/plugins/isoliner3d/ , https://github.com/Valery35/qgis-isoliner3d/releases/tag/1.5.7 |
| Isoliner Scene, Windows и браузер | 0.4.0 | https://valery35.github.io/isoliner-scene/app/ , https://github.com/Valery35/isoliner-scene/releases/tag/v0.4.0 |
| Isoliner3D для RustGIS | 0.5.1, десять инструментов, своя вкладка ленты | https://github.com/Valery35/isoliner-scene/releases/tag/rustgis-v0.5.1 |
| Страница линейки | - | https://www.informpp.ru/главная-страница/isoliner3d |

Каталог QGIS показывает 1.5.7 (проверено 15.11). На сайте страница
линейки и модуля свежие, подстраница «Isoliner3D для RustGIS» ещё
говорит о 0.5.0 - её обновление готово в исходниках, ждёт публикации.

### Снимки

- `C:\Dev\isoliner-scene\site\scene_demo.png` - демо-сцена (ствол, штрек
  со сбойкой, кровля пласта).
- `C:\Dev\isoliner-scene\site\rustgis_scene.jpg` - сцена из слоёв проекта
  RustGIS рядом с панелью инструмента 2.24.
- `C:\Dev\isoliner-scene\site\rustgis_tool.jpg` - слой «Тела выработок»
  в RustGIS после 2.24.
- В общей папке заметок (`%LOCALAPPDATA%\Claude-Data\...\side-session-notes\local_96742dae-...`)
  - `scene_shadows.png`, `scene_shadows_crop.png` - тени от Солнца в Isoliner Scene
  - `toolbar_pro_vs_lite.png` - одна плашка в модуле и в Scene
  - `scene-params-camera.jpg`, `scene-params-box.jpg` - окно «Параметры сцены».

- `C:\Dev\isoliner-scene\site\rustgis_tab_isoliner3d.jpg` - своя вкладка
  Isoliner3D на ленте RustGIS 0.5.1, группа «Сцена» и раскрытое меню
  «Оболочки» с 2.11, 2.12, 2.14, 2.23.

Видео не делали, для Reddit хватит снимков. К посту r/gis хорошо идут
`scene_shadows.png` и `rustgis_tab_isoliner3d.jpg` вместе.

## Чем Isoliner3D цепляет

1. **Одни расчёты в трёх средах.** Инструмент описан один раз. Модуль
   QGIS, расширение RustGIS и сцена считают по этому описанию, и итоги
   Rust сверены с модулем до 1e-9. Горная геология перестаёт быть
   привязанной к одной ГИС.
2. **От скважин до объёма и денег в бесплатной ГИС.** Скважины и пробы,
   кригинг и кубы, оболочки рудных тел, выработки со сводом, отчёт по
   уступам с рудой, вскрышей и маржой, и всё видно в 3D-сцене с тенями,
   которую можно открыть в браузере по ссылке с камерой.

Первое - для OSGeo и разработчиков, второе - для r/gis, r/QGIS и
геологов.

## OSGeo Discourse - ответ в теме 155878

Isoliner3D now lives in three places with one set of calculations.

- **QGIS plugin 1.5.7.** 35 Processing tools for mining geology - kriging
  and multilevel B-spline cubes, ore shells and boolean operations, mine
  workings with arched sections, borehole traces and composites, a bench
  report of a block model with ore, waste, metal and margin, 3D density.
  One compact Scene settings window with sun and shadows.
- **Isoliner Scene 0.4.0.** The same 3D window as a standalone program
  for Windows and in the browser. It opens the .i3dscene package the
  plugin exports, and a link keeps the camera.
- **Isoliner3D for RustGIS 0.5.1.** Ten of the plugin tools in the
  RustGIS Tools pane and its own ribbon tab. Each tool is described once
  in a shared tools.json, and the Rust results are checked against the
  plugin to 1e-9. 3D results open in the same scene window.

The common base for RustGIS plugins is published as rustgis-kit
(https://github.com/Valery35/rustgis-kit), and Isoliner, Topoliner and
Routeliner are moving to RustGIS on it.

Plugin: https://plugins.qgis.org/plugins/isoliner3d/
Scene in the browser: https://valery35.github.io/isoliner-scene/app/
RustGIS extension: https://github.com/Valery35/isoliner-scene/releases/tag/rustgis-v0.5.1

## Форум OSM

Поста нет. Isoliner3D не работает с данными OSM.

## linux.org.ru, раздел Open Source

Заголовок: Isoliner3D - горная геология в QGIS, RustGIS и в браузере

Ссылка: https://www.informpp.ru/главная-страница/isoliner3d
Текст ссылки: Страница линейки
Теги: qgis, gis, rust, wgpu, геология

Линейка Isoliner3D теперь работает в трёх средах и считает одинаково.

- Модуль QGIS 1.5.7 (Python, GPL-2.0-or-later). 35 инструментов
  Processing для горной геологии - кригинг и кубы, оболочки рудных тел и
  булевы операции с ними, выработки со сводом, стволы и композиты
  скважин, отчёт по уступам блочной модели, 3D-плотность. Окно просмотра
  с тенями от Солнца.
- Isoliner Scene 0.4.0 (Rust, eframe и wgpu, Apache 2.0). То же окно
  сцены отдельной программой для Windows и страницей в браузере, пакет
  сцены .i3dscene, ссылка хранит камеру.
- Isoliner3D для RustGIS 0.5.1. Десять инструментов модуля в панели
  RustGIS и своя вкладка на ленте. Инструмент описан один раз, итоги на
  Rust сверены с модулем до 1e-9.

Общая основа расширений RustGIS выложена отдельным крейтом rustgis-kit,
на нём переходят в RustGIS Isoliner, Topoliner и Routeliner.

Каталог QGIS: https://plugins.qgis.org/plugins/isoliner3d/
Сцена в браузере: https://valery35.github.io/isoliner-scene/app/
Исходники: https://github.com/Valery35/qgis-isoliner3d ,
https://github.com/Valery35/isoliner-scene

## Reddit, r/gis (публикует автор)

Заголовок: From boreholes to ore shells and a bench report - free 3D mining geology in QGIS, RustGIS and the browser

Текст:

Isoliner3D is a free toolset for mining geology. It started as a QGIS
plugin and now runs the same calculations in three places.

- **QGIS plugin.** Boreholes and samples, kriging and B-spline cubes, ore
  shells with boolean operations, mine workings with arched sections, a
  bench report of a block model (ore, waste, metal, margin) and 3D
  density. A 3D window with sun and shadows.
- **Isoliner Scene.** The same 3D scene as a Windows program and in the
  browser. Export from QGIS, open anywhere, share a link that keeps the
  camera.
- **RustGIS extension.** Ten of the tools in the RustGIS Tools pane,
  checked against the plugin to 1e-9.

Try the demo scene in the browser: https://valery35.github.io/isoliner-scene/app/
Plugin: https://plugins.qgis.org/plugins/isoliner3d/

Картинка к посту - `scene_shadows.png` или `scene_demo.png`.

## Reddit, r/QGIS (публикует автор)

Заголовок: Isoliner3D 1.5.7 - a 3D window for geology in QGIS, now with sun, shadows and a scene you can open in the browser

Текст:

Isoliner3D is a free QGIS plugin for geological surfaces, bed bodies,
boreholes, block models and mine workings in 3D. It does not depend on
the built-in 3D map.

Recent releases:

- **One Scene settings window** instead of a column of buttons. Exaggeration,
  camera, box, clipping and lighting live there.
- **Sun and shadows.** The terrain and the bodies cast shadows on each
  other, hillshade follows the same sun.
- **35 Processing tools.** New - a bench report of a block model, 3D
  density of features, bodies from outlines with a floor and a crown.
- **Export to .i3dscene.** The scene opens in Isoliner Scene for Windows
  or in the browser, without QGIS.

Plugin: https://plugins.qgis.org/plugins/isoliner3d/
Scene in the browser: https://valery35.github.io/isoliner-scene/app/

Картинка к посту - `scene_shadows.png` и `toolbar_pro_vs_lite.png`.

## Витрины

- **AlternativeTo.** Карточка Isoliner Scene - «free 3D viewer for
  geological and mining scenes, Windows and web». Альтернатива для
  Leapfrog Viewer, Micromine Viewer, Datamine Studio RM viewer. Заявка
  от аккаунта автора.
- **winget.** Isoliner Scene для Windows подходит, это zip с exe без
  установщика. Для winget нужен манифест portable, его соберёт сессия
  сцены по слову автора.
- **Flathub, Homebrew** - пока нет сборок для Linux и macOS, не подаём.
