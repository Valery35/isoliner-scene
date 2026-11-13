# Параметры сцены: общий список модуля и Isoliner Scene

Окно **Параметры сцены** (английская подпись **Scene settings**,
подсказка кнопки «Параметры сцены: вид, камера, оформление, короб,
обрезка» / «Scene settings: view, camera, look, box, clipping») одно
и то же в модуле Isoliner3D (Про)
и в программе Isoliner Scene (Лайт). Открывается одной кнопкой
на плашке значков. Всё, что меняет вид сцены, живёт здесь, а на
плашке остаются только действия.

Список ведёт модуль, Isoliner Scene его повторяет. Ключ - имя поля
в `state` файла `scene.json` пакета `.i3dscene`, камера дублируется
в корневом `camera` в настоящих координатах. Пометка «Про» значит,
что параметр есть только в модуле: в Лайт поля нет или оно только
для чтения.

Окно разбито на четыре вкладки в этом порядке. Высота окна не больше
700 пикселей при масштабе 100 процентов.

## 1. Вид / View

| Поле | Field | Ключ | Где |
|---|---|---|---|
| Вертикальное преувеличение | Vertical exaggeration | `vex` | оба |
| Разнос по Z, м | Z spacing, m | `spacing` | Про |
| Прозрачность, % | Transparency, % | `opacity` | оба |
| Азимут, ° | Azimuth, ° | `camera.azimuth` | оба |
| Наклон, ° | Elevation, ° | `camera.elevation` | оба |
| Удаление, м | Distance, m | `camera.distance` | оба |
| Угол обзора, ° | Field of view, ° | `camera.fov` | оба |
| Параллельная проекция | Orthographic projection | `camera.ortho` | оба |
| Обновлять автоматически | Rebuild automatically | нет | Про |

## 2. Оформление / Look

| Поле | Field | Ключ | Где |
|---|---|---|---|
| Отмывка, % | Hillshade, % | `light` | оба |
| Градиентный фон | Gradient background | `bg_grad` | оба |
| Сглаживать края | Smooth edges | `smooth_edges` | оба |
| Показывать разметку | Show sketch | `sketch_on` | оба |
| Тема | Theme | нет | Лайт |
| Текстура, пикс. | Texture, px | `texside` | Про |
| Предел вершин, тыс. | Vertex limit, thousands | `vert_cap` | Про |

Тема светлая или тёмная - свойство программы, а не сцены, в пакет
не пишется.

## 3. Короб / Box

| Поле | Field | Ключ | Где |
|---|---|---|---|
| Координатный короб (галка группы) | Coordinate box | `axes_on` | оба |
| Рёбра: все, три оси, без рёбер | Edges: all, three axes, none | `box_edges` (`all`, `axes`, `none`) | оба |
| Подписи к ближним | Labels on near edges | `box_near` | оба |
| Сетка: только короб, пол, пол и стены, стены | Grid: box only, floor, floor and walls, walls | `grid_planes` (`""`, `floor`, `floor,walls`, `walls`) | оба |
| Шаг сетки, м | Grid step, m | `grid_step` | оба |
| Шаг делений: план, Z | Tick step: plan, Z | `tick_xy`, `tick_z` | оба |
| Множитель подписей: план, Z | Label factor: plan, Z | `mult_xy`, `mult_z` | оба |
| Свой охват | Custom extent | `box_range_on` | оба |
| X, Y, Z от и до | X, Y, Z from and to | `box_range` (6 чисел: x0 x1 y0 y1 z0 z1) | оба |

Ноль в шаге значит «круглый шаг от размаха».

## 4. Обрезка / Clip

| Поле | Field | Ключ | Где |
|---|---|---|---|
| Контур или линия | Polygon or line | `clip` | Про, в Лайт свой контур |
| Что оставить: внутри, снаружи, слева, справа, коридор | Keep: inside, outside, left, right, corridor | `side` (`in`, `out`, `left`, `right`, `corridor`) | оба |
| Полуширина коридора, м | Corridor half-width, m | `width` | оба |
| По отметке: от, до | By elevation: from, to | `zlo`, `zhi` (-1e7 - нет) | оба |
| Сверху и снизу (растры) | Top and bottom (rasters) | `zs_top`, `zs_bot` | Про |
| Маска и порог | Mask and threshold | `mask_lyr`, `mask_level` | Про |
| Заборы по линии | Fences along the line | `fence_all` | Про |

## Плашка значков после переноса

Остаются действия: обновить, вид сверху, домой, вид по разрезу,
рисование контура и его кнопки, снять обрезку, сохранить контур,
оболочки в слой, выгрузка, открыть в Isoliner Scene, вращение,
виды облёта, видео, копия кадра, PNG и кнопка **Параметры сцены**.

Уходят в окно: параллельная проекция, координатный короб, показ
разметки, поле вертикального преувеличения.
