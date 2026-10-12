#!/usr/bin/env python3
"""Put the Creative Studio loading screen (the app's flying star) into each app's web index.html.

Run from ~/Documents/CreativeStudio after syncing an app with upstream (the web index.html files are
upstream files). Idempotent: a page that already has the splash is left alone.

    python3 rsm/studio/splash/apply.py
"""
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))

# app: (index.html, colour, how the loading text appears in upstream's page)
APPS = {
    "rsm": ("rsm/apps/vectorcraft-web/index.html", "#eb1b3c"),
    "ttshat": ("ttshat/apps/lightcraft-web/index.html", "#eb1b3c"),
    "tlween": ("tlween/apps/photocraft-web/index.html", "#eb1b3c"),
    "tsweer": ("tsweer/apps/filmcraft-web/web/index.html", "#eb1b3c"),
    "effectat": ("effectat/apps/effectcraft-web/web/index.html", "#eb1b3c"),
    "sf7aat": ("sf7aat/apps/pdfcraft-web/index.html", "#eb1b3c"),
    "trteeb": ("trteeb/apps/designcraft-web/index.html", "#eb1b3c"),
    "klmat": ("klmat/apps/wordcraft-web/index.html", "#eb1b3c"),
    "arqam": ("arqam/apps/gridcraft-web/index.html", "#eb1b3c"),
    "shoof": ("shoof/apps/deckcraft-web/index.html", "#eb1b3c"),
}

CSS = """
    /* Creative Studio loading screen: the flying star */
    .ss-scene { position: relative; width: 300px; height: 230px; --c: %s; }
    .ss-shoe { position: absolute; width: 96px; left: 50%%; top: 30px; margin-left: -48px; transform-origin: 50%% 55%%;
      animation: ss-fly 2.4s cubic-bezier(.3,.1,.3,1) infinite; }
    @keyframes ss-fly {
      0%% { transform: translate(200px, 120px) rotate(0deg) scale(.6); opacity: 0; }
      8%% { opacity: 1; }
      45%% { transform: translate(30px, -50px) rotate(-540deg) scale(1); }
      62%% { transform: translate(0, 70px) rotate(-760deg) scale(1); }
      67%% { transform: translate(0, 70px) rotate(-720deg) scale(1.15, .8); }
      74%% { transform: translate(0, 40px) rotate(-725deg) scale(.95, 1.05); }
      80%%, 92%% { transform: translate(0, 70px) rotate(-720deg) scale(1); opacity: 1; }
      100%% { transform: translate(-40px, 70px) rotate(-720deg) scale(.9); opacity: 0; } }
    .ss-shadow { position: absolute; left: 50%%; top: 186px; width: 90px; height: 14px; margin-left: -45px;
      background: rgba(0,0,0,.35); border-radius: 50%%; filter: blur(2px); animation: ss-shadow 2.4s cubic-bezier(.3,.1,.3,1) infinite; }
    @keyframes ss-shadow { 0%%, 30%% { transform: scale(.2); opacity: 0; } 62%%, 80%% { transform: scale(1); opacity: 1; }
      74%% { transform: scale(.7); } 100%% { transform: translateX(-40px) scale(.9); opacity: 0; } }
    .ss-whoosh { position: absolute; inset: 0; animation: ss-whoosh 2.4s linear infinite; }
    .ss-whoosh i { position: absolute; height: 4px; border-radius: 4px; background: var(--c); opacity: .7; filter: brightness(1.5); }
    .ss-whoosh i:nth-child(1) { width: 40px; right: 20px; top: 60px; }
    .ss-whoosh i:nth-child(2) { width: 26px; right: 50px; top: 80px; }
    .ss-whoosh i:nth-child(3) { width: 34px; right: 0; top: 100px; }
    @keyframes ss-whoosh { 0%%, 10%% { opacity: 0; transform: translateX(0); } 25%% { opacity: 1; } 45%%, 100%% { opacity: 0; transform: translateX(-120px); } }
    .ss-name { position: absolute; bottom: -10px; width: 100%%; text-align: center; direction: ltr; color: var(--ink, #f2f2f2);
      font: 800 32px/1 "Bricolage Grotesque", system-ui, sans-serif; letter-spacing: -.5px; }
    .ss-name b { color: var(--c); filter: brightness(1.4); }
    .ss-bar { position: absolute; bottom: -40px; left: 25%%; width: 50%%; height: 6px; border-radius: 6px; background: #ffffff1a; overflow: hidden; }
    .ss-bar span { display: block; height: 100%%; width: 40%%; background: var(--c); filter: brightness(1.4); border-radius: 6px; animation: ss-bar 1.4s ease-in-out infinite; }
    @keyframes ss-bar { 0%% { transform: translateX(-100%%); } 100%% { transform: translateX(260%%); } }
    .ss-shoe svg { width: 100%%; height: auto; display: block; }
    #status:not(.err) { display: none; }
"""

FONT = '<link href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,800&display=swap" rel="stylesheet">'


def scene(app, shoe):
    return (
        '<div class="ss-scene"><div class="ss-whoosh"><i></i><i></i><i></i></div><div class="ss-shadow"></div>'
        f'<div class="ss-shoe">{shoe}</div>'
        f'<div class="ss-name"><b>{app}</b></div><div class="ss-bar"><span></span></div></div>'
    )


def main():
    for app, (rel, colour) in APPS.items():
        logo = os.path.join(ROOT, "logos", "Creative_Studio", ("saf7at" if app == "sf7aat" else app) + ".svg")
        shoe = re.sub(r"<\?xml[^>]*\?>\s*", "", open(logo).read()).strip()
        shoe = re.sub(r"<title>.*?</title>", "", shoe, flags=re.S)
        path = os.path.join(ROOT, rel)
        src = open(path).read()
        s = scene(app, shoe)
        if "ss-scene" in src:
            # Swap in the current scene and colour, keep everything else.
            new = re.sub(r'<div class="ss-scene">.*?<div class="ss-bar"><span></span></div></div>', lambda m: s, src, count=1, flags=re.S)
            new = re.sub(r"(\.ss-scene \{[^}]*--c: )#[0-9a-f]{6}", r"\g<1>" + colour, new)
            if new != src:
                open(path, "w").write(new)
            print(app, "updated" if new != src else "already current")
            continue
        loading = re.compile(r"(<div id=\"\w*loading\">)(\s*(?:<p>)?Loading (?:Creative Studio )?\w+(?:&hellip;|…)(?:</p>)?)")
        img = re.compile(r'(<div id="loading">)<img [^>]*>')
        if img.search(src):
            src = img.sub(lambda m: m.group(1) + s, src, count=1)
        elif loading.search(src):
            src = loading.sub(lambda m: m.group(1) + s, src, count=1)
        else:
            # No loading element upstream (PdfCraft): add one, gone once trunk has started the app.
            box = (
                '<div id="ss_loading" style="position:fixed;inset:0;display:flex;align-items:center;'
                f'justify-content:center;pointer-events:none">{s}</div>\n'
                '<script>addEventListener("TrunkApplicationStarted", () => setTimeout(() => '
                'document.getElementById("ss_loading")?.remove(), 300));</script>\n'
            )
            src = src.replace("</body>", box + "</body>", 1)
        css = CSS % colour
        if app == "sf7aat":  # PdfCraft's page is light
            css += "    .ss-scene { --ink: #1a1a1a; }\n    .ss-shadow { opacity: .5; }\n"
        src = src.replace("</style>", css + "  </style>", 1)
        src = src.replace("</head>", "  " + FONT + "\n</head>", 1)
        open(path, "w").write(src)
        print(app, "ok")


main()
