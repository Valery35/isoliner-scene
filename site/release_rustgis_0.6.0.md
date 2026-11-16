Isoliner3D для RustGIS 0.6.0 собирает сцену прямо из слоёв карты RustGIS в системе координат карты, и ему нужен RustGIS 0.7.0 или новее.

- RustGIS 0.7.0 отдаёт расширению слои сразу в системе координат карты. Промежуточный Shapefile во временной папке, через который раньше шли слои, больше не нужен.
- Расширение работает сессией RustGIS 0.7.0, и открытая сцена из слоёв сама перечитывает слои, когда они меняются на карте, без нового нажатия кнопки.
- На прежних выпусках RustGIS пакет помечается как требующий более нового RustGIS и не скачивается. Сначала стоит обновить RustGIS до 0.7.0.
- Пакет собран на GitHub по тегу выпуска после прогона тестов, а не на машине разработчика.

## Установка

Пакет `isoliner3d-0.6.0.rgplugin` ставится на вкладке «Модули» кнопкой «Установить из файла». Прежнюю версию расширения он заменяет сам. Из командной строки расширение ставится командой `rustgis-cli plugins install isoliner3d-0.6.0.rgplugin`.

---

Isoliner3D for RustGIS 0.6.0 builds the scene straight from the RustGIS map layers in the map's coordinate system. It needs RustGIS 0.7.0 or later.

- RustGIS 0.7.0 gives the extension the layers in the map's coordinate system at once. The interim Shapefile in a temporary folder that the layers used to pass through is gone.
- The extension runs as a RustGIS 0.7.0 session, and an open scene of the layers reads them again by itself when they change on the map, with no new press of the button.
- On earlier RustGIS releases the package is marked as needing a newer RustGIS and is not downloaded. Update RustGIS to 0.7.0 first.
- The package is built on GitHub from the release tag after the tests run, not on the developer's machine.

## Install

The `isoliner3d-0.6.0.rgplugin` package installs on the Plugins tab with Install from file. It replaces the previous version of the extension by itself. From the command line the extension installs with `rustgis-cli plugins install isoliner3d-0.6.0.rgplugin`.
