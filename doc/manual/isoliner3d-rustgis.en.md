<!-- Собрано тестом manual.rs расширения isoliner-rustgis. Правки - в doc/manual/rustgis.*.md и в tools.json модуля. -->

# Isoliner3D in RustGIS

Isoliner3D for RustGIS is a RustGIS plugin with the same scene and the
same tools as the QGIS plugin. It runs in RustGIS for Windows from
version 0.5.

## Installation

1. Download `isoliner3d-0.3.0.rgplugin` from the release page.
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
rustgis-cli plugins install isoliner3d-0.3.0.rgplugin
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
rustgis-cli run --plugin isoliner3d-0.3.0.rgplugin --data axes.geojson ^
  --cmd "gp.run={\"tool\":\"plugin:isoliner3d/sweep-body\",\"params\":{\"input\":\"axes\",\"elev\":-300}}"
```

The scene window is described in the View Window chapter of the Isoliner3D manual. It is common to the QGIS plugin and the Isoliner Scene program.

# Tools

## 2.24 Body along a mine working

Builds a closed body along a line: the section is set across the axis at each of its vertices and swept along it, the ends are closed. A drift, a chamber, a decline, a shaft, a trench.

The section is a rectangle with an arch, a rectangle or a circle. The width and the height are given by a number or by a field of the line, so each working can have its own section.

The cross axis of the section is always horizontal: an inclined working keeps vertical walls, as a real decline does. At a bend of the axis the section stands on the bisector and is widened so that the wall does not narrow on the turn. A bend sharper than 100° makes the body intersect itself, and the log warns about it.

The bodies are closed: the volume, the samples inside and filling with water are counted by 2.11, 2.12, 2.14 and 2.23.

| Field | What it sets |
|---|---|
| **Working axes (lines)** | Working axes: lines. With a height in the geometry their own elevations are taken, without it the elevation comes from a field or the constant below. A survey along the floor, a design axis, a trench line. |
| **Axis elevation field (for lines without Z)** | Axis elevation field for lines without Z. Empty takes the constant elevation from the next field. |
| **Axis elevation for lines without Z, m** | Axis elevation for lines without Z, m. Lines with Z do not need it. |
| **Section** | Shape of the section. A rectangle with an arch is a drift and a chamber of a potash mine. A rectangle is a trench, a cut, a chamber with a flat roof. A circle is a shaft or a tunnel of a boring machine. |
| **Width (diameter for a circle), m** | Section width, m, the diameter for a circle. Take the design width or the surveyed mean: the volume grows in direct proportion to it. |
| **Width field** | Width field, when the sections differ. An empty field or value takes the width from the field above. |
| **Wall height, m** | Wall height, m. For a rectangle it is the whole height. A circle does not need it. |
| **Wall height field** | Wall height field. Empty takes the height from the field above. |
| **Arch rise, m (0 - semicircular)** | Rise of the arch above the walls, m. Zero gives a semicircular arch, the rise is half the width. A smaller rise gives a flat arch. |
| **Axis in the section** | Where the axis lies in the section. On the floor is how workings are surveyed along the floor. In the centre is a design axis line or a shaft axis. |
| **Faces on a circle and an arch** | Number of faces on a circle and an arch. More is smoother but makes the scene heavier. |
| **Working bodies** | Working bodies, one per line, with all the fields of the axis and the fields of length, section area and volume. The bodies are closed, they are taken by 2.11, 2.12, 2.14 and 2.23. |

## 2.17 Borehole traces from downhole surveys

Builds borehole traces from collars and downhole surveys the way they are exported from a database: a collar layer and a survey table, matched by the hole ID.

Between stations the trace follows a circular arc whose end tangents match the surveys. This is the minimum curvature method used by Micromine, Datamine, Surpac and Leapfrog, and the traces agree with theirs. From the collar to the first station the trace follows the direction from the hole passport: the azimuth and angle at the collar, when the collar layer has such fields. An inclined hole is angled from the collar itself, while the first instrument station may be deeper. Without a passport the trace follows the first station up to it. A station at depth 0 takes precedence over the passport. Past the last station the trace goes along it to the end of hole. A hole without a survey and without a passport is vertical, with a warning. An upward hole from an underground working is set by an upward angle: with the angle from the horizontal and down negative it is a positive angle.

Databases record the angle in three ways: from the horizontal with down negative, where -90 means straight down, from the horizontal with down positive, and a zenith angle from the vertical, as in Russian surveys. The convention is chosen explicitly. A wrong convention sends the trace upwards or lays it flat, so check the first hole against a known one.

The azimuth is taken from the grid north of the projection. The meridian convergence is not applied: if the azimuths in the database are true, correct them before the run.

The output is lines with Z. The length of each equals the depth along the hole, so 2.18 cuts a trace by depths without a separate table.

| Field | What it sets |
|---|---|
| **Collars** | Collars: a point layer or a table with coordinates. A point layer gives the coordinates from the geometry, a table from the X and Y fields. |
| **Hole ID field** | The hole ID field in the collars. It matches a collar with its survey. Empty - recognised by name: HOLE, HOLEID, BHID. |
| **X field (without geometry)** | The X field when the collar layer has no geometry. Not needed for a point layer. |
| **Y field (without geometry)** | The Y field when the collar layer has no geometry. Not needed for a point layer. |
| **Collar elevation field (empty - Z of the geometry)** | The collar elevation field. Empty - the elevation comes from the Z of the geometry. Without it the trace starts at zero, with a warning. |
| **Hole depth field** | The field of the hole depth (end of hole). Empty - recognised by name (EOH, DEPTH), and without such a field the trace ends at the last survey station. |
| **Collar azimuth field (empty - by name)** | The hole azimuth at the collar from the passport. An inclined hole follows it from the collar to the first station. Empty - recognised by name (AZIMUTH, AZI, BRG). |
| **Collar angle field (empty - by name)** | The hole angle at the collar from the passport, in the same convention as the survey. Empty - recognised by name (DIP, ANGLE). A station at depth 0 takes precedence over the passport. |
| **Downhole survey (table)** | The survey table: hole, depth along the hole, azimuth, angle. Fields that are not chosen are recognised by Micromine, Datamine and Surpac names. Empty - all holes are vertical. |
| **Hole ID field in the survey** | The hole ID field in the survey. IDs are matched with the collars as text without leading and trailing spaces. |
| **Survey depth field** | The field of the survey depth along the hole, metres from the collar, not an elevation and not a vertical depth. |
| **Azimuth field** | The azimuth field, degrees clockwise from grid north. The meridian convergence is not applied. |
| **Angle field** | The inclination angle field, in the convention chosen below. |
| **How the angle is recorded** | How the angle is recorded. A wrong convention sends the trace upwards or lays it flat, so check it on one known hole. |
| **Trace node step, m** | The step of trace nodes along the arc, metres. The trace is drawn by the nodes, and 2.18 finds points at depths by them too. |
| **Borehole traces** | Borehole traces: lines with true Z from the collar to the end of hole. The line length equals the depth along the hole, so 2.18 cuts it into intervals without a depth table. |
