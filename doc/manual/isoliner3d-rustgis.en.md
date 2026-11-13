<!-- Собрано тестом manual.rs расширения isoliner-rustgis. Правки - в doc/manual/rustgis.*.md и в tools.json модуля. -->

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
rustgis-cli run --plugin isoliner3d-0.4.0.rgplugin --data axes.geojson ^
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

## 2.26 Crowned bodies from outlines

Builds a closed body for each outline in plan: a flat floor, walls and a crown over them. A chamber, a lens, a dome, a dump over its base outline.

The floor is a least-squares plane through the outline elevations, so a small tilt and scatter are allowed. An outline without elevation takes the floor from a field or a constant elevation.

The crown height at a point grows with the distance to the outline. A narrow part of the body comes out low, a wide part high, and the crest follows the middle line of the shape rather than one axis: an L-shaped outline gets a crown along both arms. A linear crown is a gable roof with the rise "steepness per metre from the outline", a rounded one is an arc of the same height, like a chamber vault. The rise limit cuts the crest: a wide body gets a flat roof over its middle.

The body is built on a grid with the cell step, and the boundary runs through the centres of the cells inside the outline, so the volume is a little less than the outline area times the height. A finer cell is closer to the outline.

The bodies are closed: 2.11, 2.12, 2.14 and 2.23 compute the volume, the samples inside and the water filling.

The idea comes from the QGIS 3D Studio plugin.

| Field | What it sets |
|---|---|
| **Body outlines (polygons)** | Body outlines in plan: polygons with or without elevation. A vertical outline, as on a section, is skipped: it has no area in plan. |
| **Floor elevation field** | Floor elevation from a field. When set, the floor is level at this elevation even if the outline has its own Z. |
| **Floor elevation for outlines without Z, m** | Floor elevation for outlines without elevation and without an elevation field. Outlines with Z keep the floor on their own elevations. |
| **Wall height, m** | Height of the vertical walls from the floor to the start of the crown. Zero starts the crown right at the floor, like a lens. |
| **Crown profile** | Linear is a gable roof whose rise grows evenly. Rounded is an arc from the wall to a crest of the same height, like a chamber vault, with a larger volume. |
| **Crown steepness, m per metre from the outline** | Crown rise per metre of distance from the outline. 0.45 gives a crest of about 24 degrees, one gives 45 degrees. |
| **Crown rise at most, m (0 - no limit)** | The limit of the crown rise above the walls. Zero means no limit, and a wide body then gets a high crown. |
| **Cell, m (0 - from outline size)** | The body grid step. Zero takes about one hundred twenty cells along the long side of the outline. Finer gives a more exact boundary but more faces. |
| **Bodies from outlines** | Bodies, one per outline, with all outline fields and the fields of volume, area, elevations and crown rise. The bodies are closed, 2.11, 2.12, 2.14 and 2.23 take them. |

## 1.09 Bench report

Cuts the block model into horizontal benches and computes for each: rock mass, ore and waste, mean ore grade, metal, stripping ratio, revenue, cost and margin. The last row is the total.

Benches go from the base elevation with the bench height step, the report runs top down, the way a pit is mined. A block crossing a bench boundary is split between benches by the share of its height. The block height comes from the top and bottom fields (z_from and z_to of 1.03), otherwise from the dz field (of 2.03) around the point elevation, otherwise the whole block goes to the bench of its centre.

Ore is blocks with a grade not below the cut-off. A block without grade counts as waste, and their number goes to the log.

Metal depends on the grade unit: for percent it is in tonnes, for g/t in grams. The price is per the same metal unit. Revenue is metal with recovery at the price, cost is mining of all rock plus processing of ore. Zero economics gives only tonnage and metal.

The volume and density fields are guessed by the names vol and dens, as 1.03 and 2.03 write them. A foreign Micromine, Datamine or Surpac model goes first to a cube with 2.20, then to blocks with 2.03, or set the fields yourself.

The idea comes from the QGIS 3D Studio plugin.

| Field | What it sets |
|---|---|
| **Block model (points with Z)** | Block centres as points with elevation: the output of 1.03 or 2.03, or your own model with volume and grade fields. |
| **Field of the grade** | The grade of the useful component in the block. The unit is in the field below, the metal unit depends on it. |
| **Grade unit** | Percent gives metal in tonnes, g/t gives metal in grams. The metal price below is set per the same unit. |
| **Cut-off grade** | Blocks with a grade not below this are ore, the rest is waste. |
| **Bench height, m** | Bench height in metres. A smaller bench gives a finer report, but more rows and more split blocks. |
| **Bench base elevation, m** | The elevation benches are counted from: bench boundaries lie on it and every bench height up and down. |
| **Block volume field (empty - vol)** | Block volume, m³. Empty takes the vol or volume field when present. |
| **Density field (empty - dens)** | Block density, t/m³. Empty takes the dens or density field when present, otherwise the constant below. |
| **Density, t/m³, if no field** | Density for blocks that have no density field or an empty one. |
| **Block top field (empty - z_from)** | Block top elevation. Empty takes the z_from field when present. Without top and bottom the block height comes from the dz field around the point elevation. |
| **Block bottom field (empty - z_to)** | Block bottom elevation. Empty takes the z_to field when present. |
| **Price per metal unit** | The price of a tonne of metal for percent or of a gram for g/t. Zero gives no revenue. |
| **Recovery, %** | The share of metal that reaches the product. |
| **Mining, per tonne of rock** | The cost of mining a tonne of rock: ore and waste. |
| **Processing, per tonne of ore** | The cost of processing a tonne of ore. Waste does not carry it, only mining. |
| **Bench report** | A table without geometry: a row per bench top down and the total. |

## 2.25 3D density

Builds a cube whose cell holds a count, not an estimate: how many features fall into the cell, the sum or mean of their attribute, the length of lines inside the cell.

A feature is counted once: a point at its position, a line at its middle along the length, a polygon at the centre of its ring. Line length is split between cells exactly, at the crossings of each segment with the cell faces, and the sum over the cells equals the line length inside the grid. This is printed to the log.

An empty cell is zero, not a gap: zero finds is a measured fact too. Only for the mean a cell without features is empty: there is no mean of an empty set.

The cube carries the elevation marks (Z0, DZ) like every cube of the plugin: the scene window builds an isosurface and voxels from it, 2.03 turns it into a block model, 2.04 into a voxel body. So a cluster of finds or a dense net of workings becomes a body by a density threshold.

A grid step of zero takes a round step from the data extent: about fifty cells along the long side of the plan and twenty along the height.

The idea comes from the QGIS 3D Studio plugin.

| Field | What it sets |
|---|---|
| **Features with Z** | Points, lines or polygons with elevation: finds, samples, sampling intervals, workings. Features without Z are skipped and their count goes to the log. |
| **What to count in a cell** | Feature count is the density of finds or samples. Sum and mean use the attribute below. Line length, m is how many metres of workings or intervals pass through the cell. |
| **Attribute for the sum and mean** | A numeric attribute: metal, mass, weight of a find. Empty values do not take part. |
| **Cell in plan, m (0 - from extent)** | Cell size along x and y in metres. Zero takes a round step from the data extent, about fifty cells along the long side. |
| **Cell along height, m (0 - from extent)** | Cell size along the height in metres. Zero takes about twenty levels over the elevation range. |
| **Density cube** | The density cube: a band per elevation level, Z0 and DZ in the metadata. It opens in the scene window as an isosurface or voxels. |
| **Density cells (points)** | Non-empty cells as points at their centres, with all counts at once: n, sum, mean, len_m and the cell volume vol. |

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

## 2.23 Fill a body up to a volume

Finds the horizontal level below which the body holds the given volume and gives the filled part as a separate body. The «from the top» mode is the reverse: the level above which the body holds the given volume.

The questions are such. Up to what elevation does the water rise if so many cubic metres come into a working? How much goes into a chamber up to this elevation? Where does the floor pass if such a volume is taken off the top?

The body is cut into vertical columns over the plan. In each column the segments inside the body are taken, and the volume below the level adds up from their parts below it. Voids between the floors of a body do not count: two chambers one above the other fill in turn, the lower first.

The volume accuracy is limited by the column step, the elevation of the level is exact vertically. The log gets the level, the volume by columns and the exact volume of the shell: their difference shows whether the step is fine enough.

| Field | What it sets |
|---|---|
| **Body (polygons with Z)** | A closed body: a working, a chamber, a tank, a pit bowl. The whole layer goes as one body, as in 2.11. Such bodies come from the shells button of the viewer, 2.04, 2.11, 2.13 and 2.15. |
| **Fill from** | From the bottom is filling: water, backfill, brine rise from the floor. From the top is a cut: the level above which the body holds the given volume, for example the part taken out. |
| **Volume** | How much to fill: cubic metres or a share of the body in per cent, by the field below. More than the whole body fills the body whole, and the log says so. |
| **Volume given in** | What the volume is given in. Per cent are handy when the volume of the body is not known in advance. |
| **Column step, m (0 - by the extent)** | Column step over the plan, m. Zero gives about 400 thousand columns over the body extent. The volume accuracy is limited by this step, and the elevation of the level is exact: no steps vertically. The difference from the exact shell volume is printed. |
| **Filled part (body)** | The filled part as a body of cells: the lid is flat on the level, the walls are stepped by the column step. The attributes hold the level, the volume and the share of the body. The body is taken by 2.11, 2.12 and 2.14. |

## 2.18 Intervals along a borehole

Cuts borehole traces into intervals in three ways.

**Intervals from the table** put every sample row on the trace from its depth to its depth with all the fields of the table. The scene then shows the sampling as it is.

**Equal intervals** cut the trace into pieces of the given length from the collar. The last piece to the end of hole is shorter when the length does not divide evenly.

**Length-weighted composites** cut the same way and compute in each interval the mean of the table weighted by the length of overlap with a sample. A sample 0-1 m at 10 g/t and a sample 1-4 m at 2 g/t give a composite 0-4 m at 4 g/t, while a plain mean would give 6. A composite without a single sample gets an empty value, not a zero: a gap in sampling is not barren rock. The cover field shows the sampled share of the composite length.

There are two outputs. Pieces of the trace with Z are shown in the scene as tubes with the thickness and colour by grade. Interval midpoints as points with the same fields go into 2.02, 2.06 and 2.07: this is the way from the assays to a cube and shells.

| Field | What it sets |
|---|---|
| **Borehole traces (lines with Z)** | Borehole traces: lines with Z from the collar to the end of hole, for example from 2.17. The depth along the hole is the line length from its start. |
| **Hole ID field in the traces** | The hole ID field in the traces. It matches a trace with the rows of the interval table. |
| **How to cut** | Intervals from the table put every sample row on the trace as it is. Equal intervals cut the trace into samples of the given length from the collar to the end of hole. Composites cut the same way and compute in each a length-weighted mean of the table. |
| **Interval table** | The interval table: hole, from, to and values. Needed for intervals from the table and for composites. |
| **Hole ID field in the table** | The hole ID field in the interval table. IDs are matched with the traces as text. |
| **Interval from field** | The field of the interval start, metres along the hole from the collar, not an elevation. |
| **Interval to field** | The field of the interval end, metres along the hole. A row whose end is not beyond its start is skipped. |
| **Fields for composites (empty - all numeric)** | Numeric fields for composites. Empty - all numeric ones except the interval start and end. |
| **Interval length, m** | The length of an equal interval or a composite, metres. The last interval to the end of hole is shorter when the length does not divide evenly. |
| **Intervals** | Intervals: pieces of the trace with Z from the start to the end of the interval, with the from, to, length and value fields. In the scene they are shown as tubes with the thickness or colour by grade. |
| **Interval midpoints** | Interval midpoints: points with Z and the same fields. This is the input for 2.02, 2.06 and 2.07. |

## 2.12 Selection by a shell

Selects the features that fell into a closed shell and computes a summary over them.

That is how reserves are obtained: a block model, the shell of an ore body or of a mined-out zone, and out come the volume, the tonnage and the volume-weighted grade of the selection. Selecting the outside gives the remainder without building a difference of bodies.

Not only points can be selected. Lines and polygons with height go the same way: holes that pierced the body, traces of workings, shells of other bodies.

A point counts as inside by the parity rule: a ray is sent up from it, and if the shell is crossed an odd number of times the point is inside. No cells are needed for that, and the answer comes out exact rather than to within a cell, as with the boolean operations.

For lines and polygons the vertices are looked at. A segment that goes right through the body between two of its own vertices is not caught by vertices - the sampling step along is there for that.

The volume of the selected blocks is checked against the volume of the shell itself: the difference shows how coarse the model is for this body.

The shell must be closed: one that is not has no inside.

| Field | What it sets |
|---|---|
| **What to select (features with Z)** | What to select: points, lines or polygons with height. A block model from 2.03 and 1.03, samples, traces of workings, shells of other bodies - as long as the geometry has Z. |
| **The shell (polygons with Z)** | The closed shell the selection is made by: an ore body, a mined-out zone, a block of reserves. The shells button in the viewer, 2.04 and 2.11 give such shells. |
| **What to keep** | Entirely inside - every vertex of the feature is inside the shell. Crosses - at least one is: that is how holes and workings going right through the body are caught. Outside - the remainder, everything that does not cross. |
| **Sampling step along lines, m (0 - vertices)** | The sampling step along lines and around the edge of polygons. Without it only the vertices are looked at, and a segment that goes right through the body between two of its own vertices will not be found. Zero - vertices only. |
| **Field of the block volume** | The field of the block volume, if a block model is being selected. With it the volume of the selection is computed and checked against the volume of the shell itself: the difference shows how coarse the model is for this body. |
| **Field of the grade** | The field of the grade. The mean over the selection is weighted by volume rather than plain: blocks come in different sizes, and a plain mean gives the small ones too much say. |
| **Density, t/m3 (0 - no tonnage)** | The density of the ore. With it the volume turns into tonnes and the grade into the tonnage of metal. |
| **Selected features** | The selected features with all their own fields. The summary goes to the log. |

## 2.14 Zonal statistics of shells

Counts for every body how many features fell into it and writes the numbers into the attributes of the body.

Points: the number inside and the sum of an attribute. Lines: the number of lines that touched the body and the length of their parts inside. Polygons: the number of polygons with at least one vertex inside.

A line is cut by the body: a segment is intersected with the surface, the points are sorted along it, and only the part inside goes into the length. A segment that goes right through the body between its own vertices is not lost.

Zero and empty are different things. A body with nothing inside gets zero: that is a measured fact. A body that is not closed gets empty and goes into a warning: it has no inside.

Repeated runs over one layer of bodies add up: the result of the previous run is given as the layer of bodies to the next, and every run adds its own fields. A clash of names is refused unless overwriting is allowed.

The attribute gets a sum, a mean, a minimum, a maximum, a standard deviation, a median and quantiles, as chosen. Points and polygons weigh one each, lines weigh the length of their part inside the body: a sample interval half inside the body weighs half. Empty attribute values take no part.

The sum over the bodies agrees: the length of lines inside all the bodies plus the length outside equals the total length, and that is printed to the log.

The second, optional result is the objects themselves with the body they fell into: the fields zone, zone_ids and zone_n. It shows which samples gave the total of a body.

| Field | What it sets |
|---|---|
| **Bodies (polygons with Z)** | A layer of bodies: closed shells, one per feature. Every body gets its own count fields. A body that is not closed gets empty rather than zero: it has no inside, and there is nothing to count. |
| **What to count (features with Z)** | What to count: points, lines or polygons with height. A block model, holes, traces of workings, faults. |
| **Name of the result fields (empty - from the layer)** | The name for the result fields. Empty - from the name of the layer, turned into Latin letters. Suffixes are added to it: _n the number of features inside, _len the length of lines inside, _sum, _mean, _std and others by the chosen statistics. Give your own when you run the tool over one layer of bodies several times with different data. |
| **Attribute for statistics** | A numeric attribute for statistics: the metal or the volume of a block, a thickness, a grade. Empty - count only. |
| **Statistics of the attribute** | What to compute from the attribute. The sum is the reserve when the attribute is already the metal of a block. A grade needs the mean, the spread and quantiles. For lines the mean and quantiles are weighted by the length inside the body, as with composites. |
| **Allow overwriting of fields** | Allow overwriting fields of the same name. Without it a repeated run with the same name is refused: otherwise the second result would silently wipe out the first. |
| **Bodies with statistics** | The bodies with all their previous fields and the new count fields. To accumulate several runs, give the result of the previous run as the layer of bodies to the next. |
| **Body field for the objects (empty - number)** | The body field written to the objects: the name or code of the body. Empty - the number of the body feature in the layer. |
| **Objects with a body** | The objects themselves with the body they fell into: zone is the first body, zone_ids all of them separated by semicolons, zone_n how many bodies. An object outside the bodies gets empty and zero. Without this layer only the total per body is seen, not which samples gave it. |

## 2.11 Boolean operations on shells

Subtracts, unites and intersects two bodies.

This is how mining is accounted for: the shell of a mined-out chamber is subtracted from the shell of an ore body and the remaining reserves come out. An intersection gives what fell inside a zone, a union takes two bodies as one.

The work goes not over meshes but over cells: both shells are turned into the occupancy of a common cube, the operation is done over the occupancy, and the result is turned back into a body. An exact operation over meshes cuts triangles against each other and on touches regularly gives a result that is not closed, and no volume can be taken from it.

The price of this decision: the body comes out stepped, and the accuracy is limited by the cell. The error follows the surface area and falls with the cell: on a ten-metre cube a cell of 0.5 m gave a volume error of five per cent, 0.25 m two and a half, 0.1 m landed exactly. The memory meanwhile grows as a cube.

The shells on input must be closed: one that is not has no inside, and there is nothing to tell what is within. A shell that is not closed on input is refused with the object named.

| Field | What it sets |
|---|---|
| **The first body (polygons with Z)** | The first body: a closed shell from a layer of polygons with Z. The shells button in the viewer and the tool 2.04 give such layers. |
| **The second body (polygons with Z)** | The second body. For a difference this is what is cut out of the first: a mined-out chamber, a flooded zone. |
| **Action** | A difference leaves of the first body what the second does not hold. A union takes both, an intersection only the common part. |
| **Side of the cell, m** | The side of the cell. It sets the accuracy: the error follows the surface area and halves with every halving of the cell. On a ten-metre cube a cell of 0.5 m gave a volume error of 5 per cent, 0.25 m two and a half, 0.1 m landed exactly. The memory grows as a cube, so do not take it finer than needed. |
| **The result (bodies)** | The bodies of the result: polygons with Z, the volume of each in the attributes. The shell is always closed, so the volume can be computed from it. |
