# -*- coding: utf-8 -*-
# Isoliner Scene - сцена Isoliner3D без QGIS.
# Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
# SPDX-License-Identifier: Apache-2.0
"""Страницы линейки Isoliner3D для www.informpp.ru.

    python -I site/build_page.py

Раздел сайта устроен как у PlanetX: общая страница линейки «Isoliner3D»
и под ней страницы приложений - «QGIS Isoliner3D» (собирает модуль,
tools/build_site.py) и «Isoliner Scene» (собирает этот файл).
Расширение для RustGIS пока показано карточкой на общей странице.
Оформление то же, что у страницы модуля: палитра, шрифты, переключатель
языка. Итог - site/pages/isoliner3d.html и site/pages/isoliner-scene.html,
код для блока «Встроить» Google Сайтов.
"""
import base64
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = "https://www.informpp.ru/главная-страница"
PAGE = SITE + "/isoliner3d"
PRO = PAGE + "/qgis-isoliner3d"
SCENE = PAGE + "/isoliner-scene"
CATALOG = "https://plugins.qgis.org/plugins/isoliner3d/"
PRO_CODE = "https://github.com/Valery35/qgis-isoliner3d"
PRO_RELEASES = PRO_CODE + "/releases"
SCENE_CODE = "https://github.com/Valery35/isoliner-scene"
SCENE_RELEASES = SCENE_CODE + "/releases"
SCENE_WEB = "https://valery35.github.io/isoliner-scene/app/"
RUST_CODE = SCENE_CODE + "/tree/main/apps/isoliner-rustgis"
RUSTGIS = "https://github.com/opengeos/RustGIS"

# Лента выпусков: свежие сверху.
FEED = (
    ("2026-11-13", "pro", "1.5.4",
     "Кнопка (i) с окном «О программе», общим с Isoliner Scene, чёткие "
     "значки плашки на экранах с масштабом и дописанное руководство PDF.",
     "An (i) button with the About window shared with Isoliner Scene, "
     "sharp toolbar icons on scaled screens and an updated PDF manual."),
    ("2026-11-13", "pro", "1.5.3",
     "Тени от рельефа и объектов и положение Солнца в параметрах сцены. "
     "Исправлена выгрузка поверхностей, которая с 1.5.0 сдвигала рельеф. "
     "Плашка значков общая с Isoliner Scene.",
     "Shadows from the terrain and objects and the Sun position in the "
     "scene settings. Fixed the surface export that shifted the terrain "
     "since 1.5.0. The toolbar is shared with Isoliner Scene."),
    ("2026-11-13", "pro", "1.5.2",
     "Все параметры вида собраны в одно компактное окно за одной кнопкой, "
     "на плашке остались только действия.",
     "All view settings are gathered in one compact window behind a single "
     "button, the toolbar keeps only actions."),
    ("2026-11-13", "scene", "0.2.0",
     "Связь с QGIS. Кнопка модуля открывает сцену, пакет перечитывается "
     "сам, камера остаётся на месте. Обрезка с замкнутыми телами, вид по "
     "разрезу со шкалами, светлая и тёмная тема, видео MP4.",
     "Link with QGIS. The plugin button opens the scene, the package "
     "reloads by itself and the camera stays in place. Clipping with closed "
     "bodies, a section view with scales, light and dark themes, MP4 "
     "video."),
    ("2026-11-13", "pro", "1.5.1",
     "Кнопка «Открыть в Isoliner Scene» в окне просмотра. Модуль сам "
     "находит установленную программу.",
     "The «Open in Isoliner Scene» button in the viewer window. The plugin "
     "finds the installed program by itself."),
    ("2026-11-12", "pro", "1.5.0",
     "Инструменты переведены на общее описание, которое читают модуль "
     "и Isoliner Scene. Растры на входе и выходе инструментов, обрезка "
     "сцены с крышками.",
     "The tools moved to a shared description read by the plugin and by "
     "Isoliner Scene. Rasters as tool inputs and outputs, scene clipping "
     "with caps."),
    ("2026-11-12", "scene", "0.1.0",
     "Первый пробный выпуск. Пакет .i3dscene в браузере и на Windows "
     "с камерой из QGIS.",
     "The first trial release. The .i3dscene package in the browser and "
     "on Windows with the camera from QGIS."),
)

PRODUCTS = {
    "pro": ("Isoliner3D для QGIS", "Isoliner3D for QGIS"),
    "scene": ("Isoliner Scene", "Isoliner Scene"),
    "rust": ("Isoliner3D для RustGIS", "Isoliner3D for RustGIS"),
}

STYLE = """
@import url('https://fonts.googleapis.com/css2?family=Bitter:wght@500;600;700\
&family=Golos+Text:wght@400;500;600&display=swap');
:root{
  --paper:#F1F3EE; --paper-2:#E7EBE3; --ink:#16221F; --ink-soft:#4C5A55;
  --teal:#0E7C66; --teal-deep:#0A5446; --amber:#C2622C;
  --line:rgba(22,34,31,.14); --r:14px; --maxw:1080px;
  --display:'Bitter',Georgia,serif;
  --body:'Golos Text','Segoe UI',system-ui,sans-serif;
}
*{box-sizing:border-box}
body{margin:0;background:var(--paper);color:var(--ink);
  font-family:var(--body);font-size:17px;line-height:1.6;
  -webkit-font-smoothing:antialiased}
.wrap{max-width:var(--maxw);margin:0 auto;padding:0 24px}
a{color:inherit}
h1,h2,h3{font-family:var(--display);font-weight:700;line-height:1.1;margin:0;
  letter-spacing:-.01em}
p{margin:0}
.eyebrow{font-size:13px;font-weight:600;letter-spacing:.16em;
  text-transform:uppercase;color:var(--teal-deep);margin-bottom:18px;
  display:inline-flex;gap:10px;align-items:center}
.eyebrow::before{content:"";width:26px;height:2px;background:var(--amber);
  display:inline-block}
.hdr{position:sticky;top:0;z-index:20;background:rgba(241,243,238,.86);
  backdrop-filter:blur(8px);border-bottom:1px solid var(--line)}
.hdr .wrap{display:flex;align-items:center;justify-content:space-between;
  height:62px;gap:18px}
.brand{display:flex;align-items:baseline;gap:10px;
  font-family:var(--display);font-weight:700;text-decoration:none;
  font-size:20px}
.brand span{font-family:var(--body);font-weight:500;font-size:13px;
  color:var(--ink-soft)}
.right{display:flex;align-items:center;gap:22px}
.nav{display:flex;gap:22px;font-size:15px}
.nav a{text-decoration:none;color:var(--ink-soft)}
.nav a:hover{color:var(--teal-deep)}
@media(max-width:860px){.nav{display:none}}
.lang{display:flex;border:1px solid var(--line);border-radius:10px;
  overflow:hidden;font-size:13px;font-weight:600}
.lang button{border:0;background:transparent;padding:6px 11px;cursor:pointer;
  font:inherit;color:var(--ink-soft)}
.lang button.on{background:var(--teal);color:#fff}
.hero{padding:74px 0 48px}
.hero h1{font-size:clamp(34px,5vw,56px);max-width:18ch}
.hero .lead{margin-top:22px;font-size:20px;max-width:64ch;
  color:var(--ink-soft)}
.cta{margin-top:34px;display:flex;gap:14px;flex-wrap:wrap}
.btn{display:inline-block;padding:13px 22px;border-radius:var(--r);
  text-decoration:none;font-weight:600;font-size:16px}
.btn-main{background:var(--teal);color:#fff}
.btn-main:hover{background:var(--teal-deep)}
.btn-ghost{border:1px solid var(--line);color:var(--ink)}
.btn-ghost:hover{border-color:var(--teal)}
.note{margin-top:12px;font-size:14px;color:var(--ink-soft);max-width:64ch}
section{padding:56px 0;border-top:1px solid var(--line)}
section h2{font-size:clamp(26px,3.3vw,38px);max-width:24ch}
section .sub{margin-top:16px;max-width:70ch;color:var(--ink-soft)}
.trio{display:grid;grid-template-columns:repeat(3,1fr);gap:24px;
  margin-top:34px}
@media(max-width:860px){.trio{grid-template-columns:1fr}}
.card{background:var(--paper-2);border:1px solid var(--line);
  border-radius:var(--r);padding:22px 24px;display:block;
  text-decoration:none}
a.card:hover{border-color:var(--teal)}
.card .eyebrow{margin-bottom:10px}
.card h3{font-size:19px;margin-bottom:10px}
.card p{color:var(--ink-soft);font-size:16px}
.tag{display:inline-block;margin-top:12px;font-size:13px;font-weight:600;
  color:var(--amber)}
figure{margin:30px 0 0}
figure img{width:100%;height:auto;display:block;border-radius:var(--r);
  border:1px solid var(--line);background:#fff}
.shot{margin-top:38px;max-width:820px}
figcaption{margin-top:10px;font-size:14px;color:var(--ink-soft)}
.rows{margin-top:34px;border:1px solid var(--line);border-radius:var(--r);
  overflow:hidden;background:#fff}
.row{display:grid;grid-template-columns:260px 1fr;gap:16px;padding:14px 20px;
  border-bottom:1px solid var(--line)}
.row:last-child{border-bottom:0}
@media(max-width:860px){.row{grid-template-columns:1fr;gap:4px}}
.row b{font-family:var(--display);color:var(--teal-deep)}
.row span{color:var(--ink-soft)}
pre{margin:26px 0 0;padding:18px 20px;background:#fff;
  border:1px solid var(--line);border-radius:var(--r);overflow:auto;
  font-size:14px;line-height:1.5}
.ftr{padding:40px 0 60px;border-top:1px solid var(--line);
  color:var(--ink-soft);font-size:15px}
.ftr a{color:var(--teal-deep)}
"""

SCRIPT = """
function apply(lang){
  var d = TEXTS[lang] || TEXTS.ru;
  document.documentElement.lang = lang;
  document.querySelectorAll('[data-i18n]').forEach(function(el){
    var value = d[el.getAttribute('data-i18n')];
    if (value !== undefined) el.innerHTML = value;
  });
  document.querySelectorAll('.lang button').forEach(function(b){
    b.classList.toggle('on', b.getAttribute('data-lang') === lang);
  });
  try { localStorage.setItem('isoliner3d-lang', lang); } catch (e) {}
}
document.querySelectorAll('.lang button').forEach(function(b){
  b.addEventListener('click', function(){ apply(b.getAttribute('data-lang')); });
});
// Выбор языка общий со страницей модуля: ключ тот же.
var saved = null;
try { saved = localStorage.getItem('isoliner3d-lang'); } catch (e) {}
apply(saved || 'ru');
"""


class Page:
    """Тексты страницы на двух языках и куски разметки."""

    def __init__(self):
        self.T = {}

    def t(self, key, ru, en):
        if key in self.T and self.T[key] != (ru, en):
            raise ValueError("ключ %s занят другим текстом" % key)
        self.T[key] = (ru, en)
        return key

    def el(self, key, ru, en, tag="p", cls=None):
        self.t(key, ru, en)
        c = ' class="%s"' % cls if cls else ""
        return '<%s%s data-i18n="%s"></%s>' % (tag, c, key, tag)

    def section(self, key, eyebrow, h2, sub, inner):
        parts = [self.el(key + ".eyebrow", *eyebrow, tag="div", cls="eyebrow"),
                 self.el(key + ".h2", *h2, tag="h2")]
        if sub:
            parts.append(self.el(key + ".sub", *sub, cls="sub"))
        return ('<section id="%s"><div class="wrap">\n%s\n%s\n</div></section>'
                % (key, "\n".join(parts), inner))

    def rows(self, key, items):
        out = []
        for n, (h_ru, h_en, p_ru, p_en) in enumerate(items, 1):
            out.append('<div class="row">%s%s</div>' % (
                self.el("%s.%d.h" % (key, n), h_ru, h_en, tag="b"),
                self.el("%s.%d.p" % (key, n), p_ru, p_en, tag="span")))
        return '<div class="rows">%s</div>' % "\n".join(out)

    def cards(self, key, items, cls="trio"):
        out = []
        for n, (href, eyebrow, h, p, tag) in enumerate(items, 1):
            inner = ""
            if eyebrow:
                inner += self.el("%s.%d.e" % (key, n), *eyebrow, tag="div",
                                 cls="eyebrow")
            inner += self.el("%s.%d.h" % (key, n), *h, tag="h3")
            inner += self.el("%s.%d.p" % (key, n), *p)
            if tag:
                inner += self.el("%s.%d.t" % (key, n), *tag, tag="span",
                                 cls="tag")
            if href:
                out.append('<a class="card" target="_blank" href="%s">%s</a>'
                           % (href, inner))
            else:
                out.append('<div class="card">%s</div>' % inner)
        return '<div class="%s">%s</div>' % (cls, "\n".join(out))

    def btn(self, key, ru, en, href, main=False):
        self.t(key, ru, en)
        return ('<a class="btn %s" target="_blank" href="%s" data-i18n="%s"></a>'
                % ("btn-main" if main else "btn-ghost", href, key))

    def render(self, title, sub, home, nav, content):
        self.t("title", *title)
        self.t("brand.sub", *sub)
        self.t("ftr.line2", "ООО «Информ++»", "Inform++ LLC")
        self.t("ftr.line3", "Программы бесплатные и открытые.",
               "The programs are free and open source.")
        links = "\n".join('<a target="_blank" href="%s" data-i18n="%s"></a>'
                          % (href, key) for key, href in nav)
        texts = {"ru": {k: v[0] for k, v in self.T.items()},
                 "en": {k: v[1] for k, v in self.T.items()}}
        return """<!-- Isoliner3D - страница для www.informpp.ru.               -->
<!-- Собирается site/build_page.py хранилища isoliner-scene.    -->
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title data-i18n="title"></title>
<style>%s</style>
<header class="hdr"><div class="wrap">
<a class="brand" target="_blank" href="%s">Isoliner3D
<span data-i18n="brand.sub"></span></a>
<div class="right"><nav class="nav">%s</nav>
<div class="lang"><button type="button" data-lang="ru">RU</button>
<button type="button" data-lang="en">EN</button></div></div>
</div></header>
%s
<footer class="ftr"><div class="wrap">
<span data-i18n="ftr.line2"></span>,
<a target="_blank" href="https://www.informpp.ru/">www.informpp.ru</a><br>
<span data-i18n="ftr.line3"></span>
</div></footer>
<script>
var TEXTS = %s;
%s</script>
""" % (STYLE, home, links, content,
            json.dumps(texts, ensure_ascii=False), SCRIPT)


def picture():
    with open(os.path.join(HERE, "scene_demo.png"), "rb") as fh:
        return "data:image/png;base64," + base64.b64encode(fh.read()).decode()


def product_names(b):
    for key, (ru, en) in PRODUCTS.items():
        b.t("prod.%s" % key, ru, en)
    return [("prod.pro", PRO), ("prod.scene", SCENE), ("prod.rust", RUST_CODE)]


def family(b):
    nav = product_names(b)
    hero = ('<div class="wrap hero">\n%s\n%s\n%s\n<div class="cta">%s%s%s</div>\n'
            '<figure class="shot"><img alt="" src="%s">%s</figure>\n</div>'
            % (b.el("hero.eyebrow", "Линейка Isoliner3D · 3D для горных работ",
                    "The Isoliner3D family · 3D for mining", tag="div",
                    cls="eyebrow"),
               b.el("hero.h1", "Одна сцена - три среды",
                    "One scene - three homes", tag="h1"),
               b.el("hero.lead",
                    "Isoliner3D собирает тела пластов, выработки, скважины "
                    "и разрезы в одну трёхмерную сцену. Сцену строит модуль "
                    "QGIS, показывает Isoliner Scene в окне и в браузере, "
                    "а расширение RustGIS запускает инструменты модуля "
                    "из RustGIS. Пакет сцены .i3dscene один для всех трёх.",
                    "Isoliner3D brings seam bodies, mine workings, boreholes "
                    "and sections into one 3D scene. The QGIS plugin builds "
                    "the scene, Isoliner Scene shows it in a window and in a "
                    "browser, and the RustGIS extension runs the plugin tools "
                    "from RustGIS. The .i3dscene package is the same for all "
                    "three.", cls="lead"),
               b.btn("cta.web", "Открыть сцену в браузере",
                     "Open a scene in the browser", SCENE_WEB, main=True),
               b.btn("cta.pro", "Модуль в каталоге QGIS",
                     "The plugin in the QGIS catalog", CATALOG),
               b.btn("cta.code", "Isoliner Scene на GitHub",
                     "Isoliner Scene on GitHub", SCENE_CODE),
               picture(),
               b.el("hero.shot",
                    "Isoliner Scene, сцена демо из модуля. Ствол, штрек "
                    "со сбойкой и кровля пласта той же камерой, что в QGIS.",
                    "Isoliner Scene, the demo scene from the plugin. A shaft, "
                    "a drift with a crosscut and the seam roof with the same "
                    "camera as in QGIS.", tag="figcaption")))

    products = b.section(
        "products", ("Три приложения", "Three applications"),
        ("Каждое в своей среде", "Each in its own home"), None,
        b.cards("prod", (
            (PRO, PRODUCTS["pro"],
             ("Анализ и построение", "Analysis and modelling"),
             ("Модуль QGIS. Тела пластов по гридам, блочная модель, "
              "разрезы, выработки, 3D-окно с опросом точек. Больше "
              "тридцати инструментов Processing. Бесплатно, из каталога "
              "QGIS.",
              "A QGIS plugin. Seam bodies from grids, a block model, "
              "sections, mine workings, a 3D window with point probing. "
              "Over thirty Processing tools. Free, from the QGIS catalog."),
             None),
            (SCENE, PRODUCTS["scene"],
             ("Открыл и увидел", "Open and see"),
             ("Отдельная программа для Windows и та же программа "
              "страницей в браузере. Камера и виды из QGIS, измерение, "
              "обрезка, вид по разрезу, кадры для ролика. QGIS ставить "
              "не нужно.",
              "A standalone program for Windows and the same program as a "
              "web page. The camera and views from QGIS, measurement, "
              "clipping, a section view, frames for a video. No QGIS to "
              "install."),
             None),
            (RUST_CODE, PRODUCTS["rust"],
             ("Инструменты модуля в RustGIS", "Plugin tools in RustGIS"),
             ("Расширение RustGIS. Инструменты модуля считают те же "
              "тела и объёмы, что в QGIS, сцена открывается в окне "
              "Isoliner Scene.",
              "A RustGIS extension. The plugin tools compute the same bodies "
              "and volumes as in QGIS, the scene opens in the Isoliner Scene "
              "window."),
             ("В работе", "In progress")),
        )))

    choose = b.section(
        "choose", ("Какое выбрать", "Which one"),
        ("По задаче", "By the task"), None,
        b.rows("choose", (
            ("Работаю в QGIS", "I work in QGIS",
             "Isoliner3D для QGIS - построение тел и разрезов рядом с картой "
             "проекта, выгрузка сцены.",
             "Isoliner3D for QGIS - bodies and sections beside the project "
             "map, scene export."),
            ("Показать коллеге или заказчику", "Show a colleague or a client",
             "Isoliner Scene - пакет сцены открывается в браузере, ссылка "
             "хранит камеру.",
             "Isoliner Scene - the scene package opens in a browser, the link "
             "keeps the camera."),
            ("На руднике без ГИС", "At the mine, without a GIS",
             "Isoliner Scene для Windows - программа без QGIS, сцена "
             "открывается файлом.",
             "Isoliner Scene for Windows - a program without QGIS, the scene "
             "opens from a file."),
            ("Работаю в RustGIS", "I work in RustGIS",
             "Isoliner3D для RustGIS - инструменты модуля из меню RustGIS. "
             "Расширение в работе.",
             "Isoliner3D for RustGIS - the plugin tools from the RustGIS menu. "
             "The extension is in progress."))))

    b.t("lang.a", "Формат и проверки - isoliner-scene на GitHub",
        "The format and the tests - isoliner-scene on GitHub")
    language = b.section(
        "language", ("Общий язык", "One language"),
        ("Пакет .i3dscene и описание инструментов",
         "The .i3dscene package and the tool description"),
        ("Пакет сцены - ZIP с камерой, видами облёта и частями в scene.json "
         "и геометрией в GLB. Инструменты описаны в tools.json, по этому "
         "описанию модуль и Rust считают одно и то же. Образцы собирает "
         "модуль, тесты обеих сторон проверяют их на каждом изменении.",
         "The scene package is a ZIP with the camera, fly-through views and "
         "parts in scene.json and geometry in GLB. The tools are described "
         "in tools.json, by this description the plugin and Rust compute the "
         "same results. The plugin produces the samples, tests on both sides "
         "check them on every change."),
        '<p class="sub"><a target="_blank" href="%s" data-i18n="lang.a"></a>'
        '</p>' % SCENE_CODE)

    items = []
    for date, key, version, ru, en in FEED:
        day = "%s.%s" % (date[8:10], date[5:7])
        ru_name, en_name = PRODUCTS[key]
        items.append(("%s · %s %s" % (day, ru_name, version),
                      "%s · %s %s" % (day, en_name, version), ru, en))
    b.t("feed.all", "Все выпуски:", "All releases:")
    links = ('<p class="sub"><span data-i18n="feed.all"></span> '
             '<a target="_blank" href="%s" data-i18n="prod.pro"></a> · '
             '<a target="_blank" href="%s" data-i18n="prod.scene"></a></p>'
             % (PRO_RELEASES, SCENE_RELEASES))
    news = b.section("news", ("Обновления", "Updates"),
                     ("Последние выпуски", "Latest releases"), None,
                     b.rows("feed", items) + links)

    content = "\n".join([hero, products, choose, news, language])
    return b.render(("Isoliner3D - линейка · Информ++",
                     "Isoliner3D - the family · Inform++"),
                    ("3D для горных работ в трёх средах",
                     "3D for mining in three homes"), PAGE, nav, content)


def scene(b):
    nav = product_names(b)
    hero = ('<div class="wrap hero">\n%s\n%s\n%s\n<div class="cta">%s%s%s</div>\n'
            '%s\n<figure class="shot"><img alt="" src="%s">%s</figure>\n</div>'
            % (b.el("hero.eyebrow", "Линейка Isoliner3D · сцена без QGIS",
                    "The Isoliner3D family · a scene without QGIS",
                    tag="div", cls="eyebrow"),
               b.el("hero.h1", "Сцена из QGIS там, где QGIS нет",
                    "A QGIS scene where there is no QGIS", tag="h1"),
               b.el("hero.lead",
                    "Isoliner Scene открывает пакет сцены .i3dscene, который "
                    "выгружает модуль Isoliner3D, и показывает выработки, "
                    "залежи и поверхности той же камерой, что окно просмотра "
                    "в QGIS. Программа для Windows и та же программа "
                    "страницей в браузере.",
                    "Isoliner Scene opens the .i3dscene package exported by the "
                    "Isoliner3D plugin and shows mine workings, ore bodies and "
                    "surfaces with the same camera as the QGIS viewer window. "
                    "A program for Windows and the same program as a web "
                    "page.", cls="lead"),
               b.btn("cta.web", "Открыть в браузере", "Open in the browser",
                     SCENE_WEB, main=True),
               b.btn("cta.win", "Скачать для Windows", "Download for Windows",
                     SCENE_RELEASES + "/latest"),
               b.btn("cta.code", "Исходный код на GitHub",
                     "Source code on GitHub", SCENE_CODE),
               b.el("cta.note",
                    "Выпуск 0.2.0. Пакет в браузере читается на месте "
                    "и никуда не отправляется. Apache 2.0.",
                    "Release 0.2.0. In the browser the package is read "
                    "locally and is not sent anywhere. Apache 2.0.",
                    cls="note"),
               picture(),
               b.el("hero.shot",
                    "Сцена демо из модуля. В браузере она открывается сразу, "
                    "её можно вращать и облетать.",
                    "The demo scene from the plugin. In the browser it opens "
                    "at once, it can be rotated and flown through.",
                    tag="figcaption")))

    can = b.section(
        "can", ("Возможности", "Features"),
        ("Что умеет", "What it does"), None,
        b.cards("can", (
            (None, None, ("Тела, поверхности, линии",
                          "Bodies, surfaces, lines"),
             ("Выработки и залежи с цветом из пакета, кровля и почва "
              "пластов, оси выработок, точки проб. Прозрачные тела "
              "рисуются от дальних к ближним.",
              "Workings and ore bodies with colors from the package, seam "
              "roof and floor, working axes, sample points. Transparent "
              "bodies are drawn back to front."), None),
            (None, None, ("Камера и виды из QGIS", "Camera and views from QGIS"),
             ("Точка взгляда, азимут, наклон и раствор окна Isoliner3D. "
              "Виды облёта, переход между ними, оборот вокруг сцены.",
              "The viewpoint, azimuth, tilt and field of view of the "
              "Isoliner3D window. Fly-through views, transitions, an orbit "
              "around the scene."), None),
            (None, None, ("Преувеличение и прозрачность",
                          "Exaggeration and opacity"),
             ("Высоты умножаются при отрисовке, координаты в пакете "
              "остаются настоящими. Части скрываются по одной.",
              "Heights are scaled when drawn, the package keeps the real "
              "coordinates. Parts are hidden one by one."), None),
            (None, None, ("Измерение и опрос", "Measurement and probing"),
             ("Длина по наклону, проложение, превышение, уклон. Площадь "
              "и периметр контура. Щелчок по телу даёт координаты "
              "и отметку.",
              "Slope length, plan length, rise, gradient. Area and perimeter "
              "of an outline. A click on a body gives coordinates and "
              "elevation."), None),
            (None, None, ("Обрезка и вид по разрезу",
                          "Clipping and section view"),
             ("Контур, сторона от линии, коридор вдоль линии, пределы "
              "отметок. Тела после обрезки закрыты крышкой. Вид по разрезу "
              "смотрит поперёк линии.",
              "An outline, a side of a line, a corridor along a line, "
              "elevation limits. Clipped bodies are closed with a cap. The "
              "section view looks across the line."), None),
            (None, None, ("Снимки и ролики", "Snapshots and videos"),
             ("Снимок PNG в размер окна, кадры облёта для ролика. "
              "В браузере ссылка хранит камеру.",
              "A PNG snapshot at the window size, fly-through frames for a "
              "video. In the browser the link keeps the camera."), None),
        )))

    how = b.section(
        "how", ("Из QGIS", "From QGIS"),
        ("Как открыть сцену", "How to open a scene"), None,
        b.rows("how", (
            ("1. В QGIS", "1. In QGIS",
             "В окне просмотра Isoliner3D нажать <b>Выгрузить сцену</b> "
             "и выбрать <b>Пакет сцены Isoliner3D</b>.",
             "In the Isoliner3D viewer press <b>Export scene</b> and choose "
             "<b>Isoliner3D scene package</b>."),
            ("2. Программа", "2. The program",
             "Открыть Isoliner Scene в браузере или распаковать архив "
             "для Windows и запустить isoliner-scene.exe.",
             "Open Isoliner Scene in the browser or unpack the Windows archive "
             "and run isoliner-scene.exe."),
            ("3. Пакет", "3. The package",
             "Нажать <b>Открыть</b> или перетащить пакет в окно.",
             "Press <b>Open</b> or drop the package into the window."))))

    fmt = b.section(
        "format", ("Формат", "Format"),
        ("Пакет сцены .i3dscene", "The .i3dscene scene package"),
        ("ZIP с описанием сцены и геометрией в GLB. Пакет пишет модуль "
         "Isoliner3D, программа читает его и сохраняет обратно без потерь. "
         "Мировые координаты хранятся в двойной точности и в видеокарту "
         "не попадают.",
         "A ZIP with the scene description and geometry in GLB. The "
         "Isoliner3D plugin writes the package, the program reads it and "
         "saves it back without loss. World coordinates are kept in double "
         "precision and never reach the graphics card."),
        "<pre>scene_demo.i3dscene\n"
        "├── scene.json   camera, views, parts, colors, exaggeration\n"
        "└── *.glb        vertices relative to the origin</pre>")

    content = "\n".join([hero, can, how, fmt])
    return b.render(("Isoliner Scene · Информ++", "Isoliner Scene · Inform++"),
                    ("сцена без QGIS", "a scene without QGIS"), PAGE, nav,
                    content)


def main():
    out = os.path.join(HERE, "pages")
    os.makedirs(out, exist_ok=True)
    for name, build in (("isoliner3d", family), ("isoliner-scene", scene)):
        text = build(Page())
        with open(os.path.join(out, name + ".html"), "wb") as fh:
            fh.write(text.encode("utf-8"))
        print("%-16s %.0f КБ" % (name, len(text.encode("utf-8")) / 1024))


if __name__ == "__main__":
    main()
