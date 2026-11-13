# Isoliner3D для RustGIS 0.5.0 - посты

Ссылки:

- выпуск https://github.com/Valery35/isoliner-scene/releases/tag/rustgis-v0.5.0
- крейт для расширений https://github.com/Valery35/rustgis-kit

## Русский (ВК, Facebook)

Isoliner3D для RustGIS 0.5.0 - десять инструментов

В панели «Инструменты» RustGIS теперь десять инструментов модуля Isoliner3D - тело вдоль выработки, тела по контуру со сводом, заполнение тела до объёма, булевы операции и выборка оболочкой, зональная статистика, стволы скважин и интервалы по ним, 3D-плотность, отчёт по горизонтам. Считают они так же, как модуль QGIS, это сверено на тех же прогонах.

Итоги складываются в цепочку. Тела выработок идут в заполнение до объёма и в выборку проб, стволы скважин - в интервалы, интервалы - в зональную статистику. Высоты итогов расширение хранит само, поэтому тела не теряют объём между шагами.

Объёмный итог сразу открывается в окне сцены, в проект RustGIS ложится его план с полями расчёта.

Общая часть расширения вынесена в открытый крейт rustgis-kit. На нём строятся расширения RustGIS для Isoliner, Topoliner и Routeliner.

Расширение - https://github.com/Valery35/isoliner-scene/releases/tag/rustgis-v0.5.0
Крейт - https://github.com/Valery35/rustgis-kit

#QGIS #RustGIS #геология #горноедело #3D #ГИС

## English (LinkedIn, Mastodon)

Isoliner3D for RustGIS 0.5.0 - ten tools

The RustGIS Tools pane now has ten tools of the Isoliner3D plugin - body along a mine working, crowned bodies from outlines, fill a body to a volume, shell booleans and selection, zonal statistics, borehole traces and intervals, 3D density and the bench report. They compute as in the QGIS plugin, checked on the same runs.

Results chain. Working bodies go to filling and sample selection, traces go to intervals, intervals go to zonal statistics. The extension keeps the heights of its results, so bodies keep their volume between steps.

A 3D result opens in the scene window at once, and its plan with the computed fields goes to the RustGIS project.

The common part is now an open crate, rustgis-kit, the base of the RustGIS extensions for Isoliner, Topoliner and Routeliner.

https://github.com/Valery35/isoliner-scene/releases/tag/rustgis-v0.5.0
https://github.com/Valery35/rustgis-kit

#QGIS #RustGIS #geology #mining #3D #GIS #opensource #rustlang

## X (до 280 знаков)

Isoliner3D for RustGIS 0.5.0 - ten QGIS plugin tools in RustGIS, results chain from step to step, 3D results open in the scene window.
#RustGIS #QGIS #mining
https://github.com/Valery35/isoliner-scene/releases/tag/rustgis-v0.5.0

## Лента сайта

RU - Isoliner3D для RustGIS 0.5.0 - десять инструментов модуля в панели RustGIS, итоги складываются в цепочку, объёмные итоги открываются в окне сцены.

EN - Isoliner3D for RustGIS 0.5.0 - ten plugin tools in the RustGIS Tools pane, results chain from step to step, 3D results open in the scene window.
