<!-- Isoliner Scene - the Isoliner3D scene without QGIS. Copyright (C) 2026 Inform++ LLC. Apache License 2.0. -->
<!-- A chapter of the Isoliner3D manual. This file is the source, manual_en.md of the plugin takes a copy between the rustgis:begin and rustgis:end marks. -->

# Isoliner3D in RustGIS

Isoliner3D for RustGIS is a RustGIS plugin with the same scene and the
same tools as the QGIS plugin. It runs in RustGIS for Windows from
version 0.5.

## Installation

1. Download `isoliner3d-0.4.0.rgplugin` from the release page.
2. In RustGIS open the Plugins tab and click Install from File.
3. Choose the package and confirm the installation.

RustGIS warns that the plugin is a program on the computer and not a
sandboxed module. All process plugins of RustGIS work this way.

After installation the Plugins tab has an Isoliner3D group with the
Isoliner3D Scene and About Isoliner3D buttons and a button per tool.
The same tools are in the RustGIS Tools pane, in the Isoliner3D
group.

Installation from the command line.

```
rustgis-cli plugins install isoliner3d-0.4.0.rgplugin
```

Removal is in the Plugin Manager, on the Isoliner3D row.

## Scene

The Isoliner3D Scene button opens the scene window with the visible
vector layers of the map. The window is the one of the Isoliner Scene
program and of the QGIS plugin's view window - the toolbar, Scene
Settings, fly-through views, video and export. The View Window chapter
describes it.

When the layers change, Isoliner3D Scene builds the scene again. An
open window reloads it by itself, a second window does not open.

A scene package `.i3dscene` written by the QGIS plugin opens with the
Open… button of the scene window.

## Tools

RustGIS has two tools now.

- 2.24 Body along a mine working.
- 2.17 Borehole traces from survey.

Their fields, hints and messages are those of the QGIS plugin. The tool
pages of the Tools chapter are common to QGIS and RustGIS.

The result goes to the project as a new layer. Undo in RustGIS
(Ctrl+Z) removes it.

## How RustGIS differs from QGIS

- A RustGIS layer keeps its coordinate system, but a plugin gets its
  features in WGS 84 degrees and without Z. The degrees go to metres of
  the UTM zone at the middle of the input layers for the run, and the
  result goes back to degrees. The run messages name the zone.
- The scene's coordinate system works like the map's. By default the
  scene is built in the RustGIS map projection when the map has an EPSG
  code in metres. A Mercator map, the globe or a system in degrees give
  a scene in metres of a UTM zone. A code in the Scene coordinate system
  (EPSG) field of Scene Settings, on the View tab, wins over the map.
  A new system applies at the next press of Isoliner3D Scene. RustGIS
  brings the layers to the scene's system.
- Heights come from fields. For 2.24 it is the axis elevation field or
  the constant elevation. For 2.17 it is the collar elevation field.
- A 2.24 body goes to the project as an outline in plan, the top faces
  of the body, with the length, section area and volume fields. The
  scene shows this layer as a body with heights.
- 2.17 takes collars as a point layer or as a table with X and Y in
  metres. Points go through the UTM zone. A table without points gives
  X and Y from its fields, and the Coordinate system of X and Y (EPSG)
  field takes their code, for example EPSG:28410. The traces then go
  to the project in that system, and RustGIS puts them in place.
- Tools with bodies on input (2.11, 2.12, 2.14, 2.23) and 2.18 with
  traces in Z stay in the QGIS plugin. A RustGIS layer holds neither
  bodies nor lines with Z.
- The scene follows the layers at the press of the button. A RustGIS
  with plugin sessions will let it follow them by itself.

## Checking without installation

A single run checks the package, RustGIS installs nothing.

```
rustgis-cli run --plugin isoliner3d-0.4.0.rgplugin --data axes.geojson ^
  --cmd "gp.run={\"tool\":\"plugin:isoliner3d/sweep-body\",\"params\":{\"input\":\"axes\",\"elev\":-300}}"
```
