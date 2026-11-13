# Запросы разработчикам RustGIS

Черновики четырёх запросов (issues) в github.com/opengeos/RustGIS от имени
автора. Публикует автор сам. Написаны по опыту расширения Isoliner3D для
RustGIS 0.4.0 на RustGIS 0.6.0, 13 ноября 2026 года. Примеры глобуса
RustPlanetX в запросах 2-4 дало управление PlanetX, просьба идёт от обеих
линеек.

---

## 1. Plugin API: layer CRS for plugins and tools

**Title:** Plugin API: expose the layer CRS in host.layers, host.features and tool inputs

Our plugin (Isoliner3D for RustGIS, a 3D mining scene and tools) needs
coordinates in metres. Today `host.features` and the layers passed to a
tool always arrive in WGS 84, and `host.layers` does not tell the layer's
own CRS, although RustGIS knows it (`share.exportLayer` keeps EPSG:32637 for
a layer read in 32637).

Request:
- `host.layers` returns each layer's CRS (EPSG or WKT);
- `host.features` and tool inputs accept an optional `crs` (for example
  `"layer"`, `"map"` or an EPSG code), so the plugin receives coordinates in
  that CRS.

Output already works well: `host.addLayer` with GeoJSON in metres and a
`crs` member is reprojected by RustGIS. The same on the input side would
let plugins skip their own reprojection.

## 2. Keep Z coordinates

**Title:** Keep Z coordinates in layers, host.features and host.addLayer

Mine workings, boreholes and ore bodies are 3D. When a layer with Z is read,
or a GeoJSON with 3D coordinates is added through `host.addLayer`, Z is
dropped (0.6.0 and main). We have to keep 3D results only in our own scene
window and give RustGIS a 2D outline.

RustPlanetX, the PlanetX 3D globe linked with the RustGIS map, would draw a
GPS or drone track and a borehole at their real heights if the layers it
reads with `host.features` kept Z. Today they are draped on the terrain. A
flight path the globe sends back with `host.addLayer` loses its altitude the
same way.

Request: keep the third coordinate in layer storage, in `host.features`
output and in `host.addLayer` / tool output, even if the 2D map ignores it.

## 3. Raster layers for plugins

**Title:** Plugin API: read raster layers (grid, extent, CRS, nodata)

Many of our tools take a grid: terrain, a seam roof and floor, a block
model. Plugins cannot read raster layers now.

RustPlanetX already shows RustGIS rasters on the globe as styled tiles from
`imagery.hostedImagery`. A DEM in the user's project could also be the
globe's terrain, if a plugin could read the raster's values.

Request: a host call that returns a raster layer as either a file path
(GeoTIFF) or a grid (width, height, extent, CRS, nodata and band values),
and the same as a tool input type.

## 4. Plugin sessions in a release

**Title:** Ship plugin sessions ("session": true) in a release

`docs/plugins.md` on main describes plugin sessions with layer events, but
0.6.0 runs every plugin command as a separate process. With sessions our
3D scene would follow layer changes live instead of being rebuilt by a
button press.

Today a live link is possible through the RustGIS MCP relay
(`127.0.0.1:8767`, `rustgis/attach` with a token), and the RustPlanetX globe
uses it. Sessions in the plugin protocol would make this simpler and
available to every plugin.

Question: is there a plan to release sessions, and is the protocol on main
stable enough to build on?
