<!-- Isoliner Scene - the Isoliner3D scene without QGIS. Copyright (C) 2026 Inform++ LLC. Apache License 2.0. -->
<!-- A chapter of the Isoliner3D manual. This file is the source, manual_en.md of the plugin takes a copy between the rustgis:begin and rustgis:end marks. -->

# Isoliner3D in RustGIS

Isoliner3D for RustGIS is a RustGIS plugin with the same scene and the
same tools as the QGIS plugin. It runs in RustGIS for Windows from
version 0.5.

## Installation

1. Download `isoliner3d-0.5.1.rgplugin` from the release page.
2. In RustGIS open the Plugins tab and click Install from File.
3. Choose the package and confirm the installation.

RustGIS warns that the plugin is a program on the computer and not a
sandboxed module. All process plugins of RustGIS work this way.

After installation the RustGIS ribbon has an Isoliner3D tab of its own.
The Scene group has the Isoliner3D Scene and About Isoliner3D buttons,
the Tools group has the Bodies, Shells, Boreholes, Reports and Density
menus. The same tools are in the RustGIS Tools pane, in the Isoliner3D
group.

Installation from the command line.

```
rustgis-cli plugins install isoliner3d-0.5.1.rgplugin
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

RustGIS has ten tools now.

- 1.09 Bench report.
- 2.24 Body along a mine working.
- 2.26 Crowned bodies from outlines.
- 2.23 Fill a body up to a volume.
- 2.11 Boolean operations on shells.
- 2.12 Selection by a shell.
- 2.14 Zonal statistics of shells.
- 2.17 Borehole traces from survey.
- 2.18 Intervals along a borehole.
- 2.25 3D density.

Their fields, hints and messages are those of the QGIS plugin. The tool
pages of the Tools chapter are common to QGIS and RustGIS.

The result goes to the project as a new layer. Undo in RustGIS
(Ctrl+Z) removes it. Bodies, traces and intervals with heights also
open in a scene window, one window per tool. An open window reloads a
new result by itself.

The 1.09 report goes to the project as a table. A RustGIS layer cannot
hold the 2.25 cube: its non-empty cells show in the scene window as
points, and the Density cells box puts them in the project as a point
layer.

Results chain. 2.24 and 2.26 bodies go to 2.23, 2.11, 2.12 and 2.14,
2.17 traces go to 2.18 and 2.25, and 2.18 intervals go to 2.12, 2.14
and 2.25. An Isoliner3D
result layer carries heights a RustGIS layer does not have. The plugin
keeps them beside the scene and knows the layer by its name and
features. A renamed or edited result layer loses its heights, and a
tool that needs bodies then says the layer has none.

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
- A body goes to the project as an outline in plan, the top faces of
  the body, with the run's fields (volume, area) and the `z_min` and
  `z_max` elevations. Traces and intervals carry the same elevations.
  The scene shows such a layer with heights.
- 2.17 takes collars as a point layer or as a table with X and Y in
  metres. Points go through the UTM zone. A table without points gives
  X and Y from its fields, and the Coordinate system of X and Y (EPSG)
  field takes their code, for example EPSG:28410. The traces then go
  to the project in that system, and RustGIS puts them in place.
- Bodies and lines with heights on input come only from Isoliner3D
  results. A body drawn in RustGIS or loaded from a file keeps no
  heights, such bodies are built in the QGIS plugin. 2.13 (a raster on
  input) stays in the plugin, RustGIS does not give rasters to plugins.
- Several items of a list (the 2.14 statistics) are typed comma
  separated, the field's hint lists the items.
- The scene follows the layers at the press of the button. A RustGIS
  with plugin sessions will let it follow them by itself.

## Checking without installation

A single run checks the package, RustGIS installs nothing.

```
rustgis-cli run --plugin isoliner3d-0.5.1.rgplugin --data axes.geojson ^
  --cmd "gp.run={\"tool\":\"plugin:isoliner3d/sweep-body\",\"params\":{\"input\":\"axes\",\"elev\":-300}}"
```
