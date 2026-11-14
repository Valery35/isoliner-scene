Isoliner3D для RustGIS 0.5.1 - своя вкладка на ленте RustGIS.

- Вкладка Isoliner3D вместо десяти одинаковых кнопок на вкладке «Модули». В группе «Сцена» кнопки сцены и «О программе», в группе «Инструменты» меню «Тела», «Оболочки», «Скважины», «Отчёты» и «Плотность».
- Итог с объектами входа начинается их полями, как у модуля QGIS. Тела 2.24 несут поля оси, тела 2.26 - поля контура, отбор 2.12 - поля объектов.
- 2.18 - поля для композитов вводятся через запятую, можно несколько.
- Пакет собран на GitHub по тегу, после тестов.

## Установка

1. Скачать `isoliner3d-0.5.1.rgplugin`.
2. В RustGIS открыть вкладку «Модули» и нажать «Установить из файла».
3. Выбрать пакет и подтвердить установку. Прежнюю версию пакет заменяет.

Нужен RustGIS 0.5.0 или новее, Windows, проверено в 0.6.0.

---

Isoliner3D for RustGIS 0.5.1 - a ribbon tab of its own.

- An Isoliner3D tab instead of ten identical buttons on the Plugins tab. The Scene group has the scene and About buttons, the Tools group has the Bodies, Shells, Boreholes, Reports and Density menus.
- A result built on input features starts with their fields, as in the QGIS plugin. 2.24 bodies carry the axis fields, 2.26 bodies the outline fields, 2.12 the fields of the selected features.
- 2.18 - fields for composites are entered comma-separated, several at once.
- The package is built on GitHub from the tag, after the tests.

## Install

1. Download `isoliner3d-0.5.1.rgplugin`.
2. In RustGIS open the Plugins tab and press Install from file.
3. Choose the package and confirm. The package replaces the previous version.

Needs RustGIS 0.5.0 or later, Windows, checked in 0.6.0.
