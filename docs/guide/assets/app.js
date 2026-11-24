// MarchingSquaresTerrain guide: shared shell (nav, search, theme) and interactive components.
// Works from file:// (no fetch). Pages include this with <script src="assets/app.js" defer>.
(function () {
  "use strict";
  const PAGES = [
    { group: "Start here" },
    { file: "index.html", n: "", title: "Welcome" },
    { file: "01-godot-for-roblox.html", n: "1", title: "Godot for Roblox builders" },
    { file: "02-first-world.html", n: "2", title: "Your first world (quick start)" },
    { group: "Terrain" },
    { file: "03-terrain-sculpting.html", n: "3", title: "Sculpting terrain" },
    { file: "04-terrain-painting.html", n: "4", title: "Painting, textures & tint" },
    { file: "05-foliage-grass.html", n: "5", title: "Foliage, grass & flowers" },
    { file: "06-chunks-settings.html", n: "6", title: "Chunks, settings & saving" },
    { group: "The pixel look" },
    { file: "07-pixelizer.html", n: "7", title: "The pixelizer" },
    { file: "08-atmosphere-effects.html", n: "8", title: "Sky, weather, water & effects" },
    { group: "Builder (modelling)" },
    { file: "09-builder-greybox.html", n: "9", title: "Greyboxing with the Roblox UI" },
    { file: "10-builder-modeller.html", n: "10", title: "Modelling: faces, edges & vertices" },
    { group: "Models" },
    { file: "11-importing-models.html", n: "11", title: "Importing models" },
    { file: "12-using-models.html", n: "12", title: "Using models: tiles, foliage & props" },
    { file: "13-playtest-export.html", n: "13", title: "Playtesting & exporting" },
    { group: "Reference" },
    { file: "14-shortcuts-glossary.html", n: "14", title: "Shortcuts & Roblox→Godot glossary" },
    { file: "15-troubleshooting.html", n: "15", title: "Troubleshooting & FAQ" },
  ];
  const here = location.pathname.split("/").pop() || "index.html";
  const store = {
    get(k) { try { return localStorage.getItem("mstguide:" + k); } catch (e) { return null; } },
    set(k, v) { try { localStorage.setItem("mstguide:" + k, v); } catch (e) { /* private mode */ } },
  };
  const el = (tag, attrs, html) => {
    const n = document.createElement(tag);
    for (const k in attrs || {}) n.setAttribute(k, attrs[k]);
    if (html != null) n.innerHTML = html;
    return n;
  };

  // ---------- theme
  const theme = store.get("theme");
  if (theme) document.documentElement.setAttribute("data-theme", theme);

  function shell() {
    if (document.body.dataset.noshell != null) return;
    const content = document.querySelector("main") || document.body;
    const top = el("header", { class: "topbar" });
    top.innerHTML =
      '<button class="iconbtn menu-toggle" aria-label="Menu">☰</button>' +
      '<a class="brand" href="index.html">⛰️ MST Guide</a><span class="spacer"></span>' +
      '<input type="search" placeholder="Search the guide…" aria-label="Search">' +
      '<button class="iconbtn theme-btn" title="Light / dark">◐</button>' +
      '<div class="search-results"></div>';
    const side = el("nav", { class: "sidebar", "aria-label": "Chapters" });
    for (const p of PAGES) {
      if (p.group) { side.appendChild(el("div", { class: "group" }, p.group)); continue; }
      const a = el("a", { href: p.file }, `<span class="num">${p.n}</span>${p.title}`);
      if (p.file === here) a.classList.add("active");
      side.appendChild(a);
    }
    const layout = el("div", { class: "layout" });
    const main = content.tagName === "MAIN" ? content : el("main");
    if (main !== content) { while (document.body.firstChild) main.appendChild(document.body.firstChild); }
    document.body.prepend(top);
    layout.appendChild(side);
    layout.appendChild(main);
    top.after(layout);
    top.querySelector(".menu-toggle").onclick = () => document.body.classList.toggle("nav-open");
    top.querySelector(".theme-btn").onclick = () => {
      const cur = document.documentElement.getAttribute("data-theme") ||
        (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
      const nxt = cur === "dark" ? "light" : "dark";
      document.documentElement.setAttribute("data-theme", nxt);
      store.set("theme", nxt);
    };
    // pager
    const list = PAGES.filter((p) => p.file);
    const i = list.findIndex((p) => p.file === here);
    const inner = main.querySelector(".content") || main;
    if (i >= 0 && !inner.querySelector(".pager")) {
      const pg = el("div", { class: "pager" });
      if (i > 0) pg.appendChild(el("a", { href: list[i - 1].file }, `<small>← Previous</small>${list[i - 1].title}`));
      if (i < list.length - 1) pg.appendChild(el("a", { class: "next", href: list[i + 1].file }, `<small>Next →</small>${list[i + 1].title}`));
      inner.appendChild(pg);
    }
    search(top);
    toc(inner);
  }

  function slug(s) { return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, ""); }

  function toc(root) {
    const hs = root.querySelectorAll("h2, h3");
    if (hs.length < 3) return;
    const box = el("nav", { class: "toc", "aria-label": "On this page" }, "<b>On this page</b>");
    const links = [];
    hs.forEach((h) => {
      if (!h.id) h.id = slug(h.textContent);
      const a = el("a", { href: "#" + h.id, class: h.tagName.toLowerCase() }, h.textContent);
      box.appendChild(a); links.push([h, a]);
    });
    document.body.appendChild(box);
    const io = new IntersectionObserver((es) => {
      es.forEach((e) => { if (e.isIntersecting) { links.forEach(([, a]) => a.classList.remove("on")); const l = links.find(([h]) => h === e.target); if (l) l[1].classList.add("on"); } });
    }, { rootMargin: "0px 0px -75% 0px" });
    links.forEach(([h]) => io.observe(h));
  }

  function search(top) {
    const input = top.querySelector("input[type=search]");
    const out = top.querySelector(".search-results");
    const idx = (window.GUIDE_INDEX || PAGES.filter((p) => p.file).map((p) => ({ file: p.file, title: p.title, heading: "", text: p.title })));
    input.addEventListener("input", () => {
      const q = input.value.trim().toLowerCase();
      out.innerHTML = "";
      if (q.length < 2) { out.classList.remove("open"); return; }
      const words = q.split(/\s+/);
      const hits = idx.filter((r) => words.every((w) => (r.title + " " + r.heading + " " + r.text).toLowerCase().includes(w))).slice(0, 30);
      for (const r of hits) {
        out.appendChild(el("a", { href: r.file + (r.id ? "#" + r.id : "") }, `${r.heading || r.title}<small>${r.title}</small>`));
      }
      if (!hits.length) out.appendChild(el("a", {}, "No matches"));
      out.classList.add("open");
    });
    document.addEventListener("click", (e) => { if (!top.contains(e.target)) out.classList.remove("open"); });
  }

  // ---------- lightbox for figure.shot images
  function lightbox() {
    const lb = el("div", { class: "lightbox", role: "dialog" }, "<img alt=''><p></p>");
    document.body.appendChild(lb);
    lb.onclick = () => lb.classList.remove("open");
    document.addEventListener("keydown", (e) => { if (e.key === "Escape") lb.classList.remove("open"); });
    document.querySelectorAll("figure.shot img, .gallery img").forEach((img) => {
      img.loading = "lazy";
      img.addEventListener("click", () => {
        lb.querySelector("img").src = img.currentSrc || img.src;
        const cap = img.closest("figure")?.querySelector("figcaption");
        lb.querySelector("p").textContent = cap ? cap.textContent : img.alt;
        lb.classList.add("open");
      });
    });
  }

  // ---------- video clips: <figure class="clip"><video src poster></video><figcaption>
  function clips() {
    document.querySelectorAll("figure.clip video").forEach((v) => {
      v.controls = true; v.muted = true; v.loop = true; v.playsInline = true; v.preload = "metadata";
      const bar = el("div", { class: "clip-controls" });
      [0.5, 1, 1.5, 2].forEach((r) => {
        const b = el("button", { type: "button" }, r + "×");
        if (r === 1) b.classList.add("on");
        b.onclick = () => { v.playbackRate = r; bar.querySelectorAll("button").forEach((x) => x.classList.toggle("on", x === b)); };
        bar.appendChild(b);
      });
      v.after(bar);
      if (v.hasAttribute("data-autoplay")) {
        new IntersectionObserver((es) => es.forEach((e) => (e.isIntersecting ? v.play().catch(() => {}) : v.pause())), { threshold: 0.5 }).observe(v);
      }
    });
  }

  // ---------- before/after: <div class="compare" data-before="Before" data-after="After"><img><img></div>
  function compares() {
    document.querySelectorAll(".compare").forEach((c) => {
      const [a, b] = c.querySelectorAll("img");
      if (!a || !b) return;
      const wrap = el("div", { class: "after-wrap" }); wrap.appendChild(b); c.appendChild(wrap);
      const bar = el("div", { class: "bar" }); c.appendChild(bar);
      c.appendChild(el("span", { class: "tag l" }, c.dataset.before || "Before"));
      c.appendChild(el("span", { class: "tag r" }, c.dataset.after || "After"));
      const set = (x) => {
        const r = c.getBoundingClientRect();
        const p = Math.min(100, Math.max(0, ((x - r.left) / r.width) * 100));
        wrap.style.clipPath = `inset(0 0 0 ${p}%)`; bar.style.left = p + "%";
      };
      let down = false;
      c.addEventListener("pointerdown", (e) => { down = true; c.setPointerCapture(e.pointerId); set(e.clientX); });
      c.addEventListener("pointermove", (e) => { if (down) set(e.clientX); });
      c.addEventListener("pointerup", () => (down = false));
    });
  }

  // ---------- scrubber: a slider that swaps frames captured at different knob values
  // <div class="scrubber" data-label="Pixel Size" data-frames='[{"v":"1","src":"media/x/ps1.png","note":"..."}, ...]'></div>
  function scrubbers() {
    document.querySelectorAll(".scrubber[data-frames]").forEach((s) => {
      let frames; try { frames = JSON.parse(s.dataset.frames); } catch (e) { return; }
      if (!frames.length) return;
      frames.forEach((f) => { const i = new Image(); i.src = f.src; });
      const start = Math.max(0, Math.min(frames.length - 1, parseInt(s.dataset.start || "0", 10)));
      s.innerHTML = `<img alt=""><div class="row"><span class="label"></span><input type="range" min="0" max="${frames.length - 1}" step="1" value="${start}"><output></output></div><div class="note"></div>`;
      const img = s.querySelector("img"), rng = s.querySelector("input"), out = s.querySelector("output"), note = s.querySelector(".note");
      s.querySelector(".label").textContent = s.dataset.label || "Value";
      const show = () => { const f = frames[+rng.value]; img.src = f.src; img.alt = (s.dataset.label || "") + " = " + f.v; out.textContent = f.v; note.textContent = f.note || ""; };
      rng.addEventListener("input", show); show();
    });
  }

  // ---------- hotspots: <div class="hotspots"><img><button class="spot" style="left:12%;top:30%" data-title="" data-text="">1</button>...</div>
  function hotspots() {
    document.querySelectorAll(".hotspots").forEach((h) => {
      const spots = [...h.querySelectorAll(".spot")];
      if (!spots.length) return;
      // Markers are % of the image: keep image + markers in their own positioned frame,
      // so the text and legend added below do not shift them.
      const img = h.querySelector(":scope > img");
      if (img) {
        const frame = el("div", { class: "hs-frame" });
        h.insertBefore(frame, img); frame.appendChild(img);
        spots.forEach((s) => frame.appendChild(s));
      }
      const text = el("div", { class: "spot-text" }, "<i>Click a numbered marker (or a list item) to learn what that part does.</i>");
      const legend = el("ol", { class: "legend" });
      const pick = (i) => {
        spots.forEach((s, j) => s.classList.toggle("on", i === j));
        [...legend.children].forEach((li, j) => li.classList.toggle("on", i === j));
        text.innerHTML = `<b>${spots[i].textContent}. ${spots[i].dataset.title || ""}</b><br>${spots[i].dataset.text || ""}`;
      };
      spots.forEach((s, i) => {
        s.type = "button"; s.title = s.dataset.title || "";
        s.onclick = () => pick(i);
        const li = el("li", {}, s.dataset.title || ""); li.onclick = () => pick(i); legend.appendChild(li);
      });
      h.appendChild(text); h.appendChild(legend);
    });
  }

  // ---------- tabs: <div class="tabs"><div class="panel" data-tab="Name">...</div>...</div>
  function tabs() {
    document.querySelectorAll(".tabs").forEach((t, ti) => {
      const panels = [...t.querySelectorAll(":scope > .panel")];
      const list = el("div", { class: "tablist", role: "tablist" });
      panels.forEach((p, i) => {
        const b = el("button", { role: "tab", type: "button", "aria-selected": i === 0 ? "true" : "false" }, p.dataset.tab || "Tab " + (i + 1));
        b.onclick = () => { panels.forEach((q, j) => (q.hidden = j !== i)); [...list.children].forEach((x, j) => x.setAttribute("aria-selected", j === i)); };
        list.appendChild(b); p.hidden = i !== 0;
      });
      t.prepend(list);
    });
  }

  // ---------- steps: <ol class="steps" data-id="unique">; ticks remembered per browser
  function steps() {
    document.querySelectorAll("ol.steps").forEach((ol, oi) => {
      const id = here + ":" + (ol.dataset.id || oi);
      let done = {}; try { done = JSON.parse(store.get("steps:" + id) || "{}"); } catch (e) { done = {}; }
      [...ol.children].forEach((li, i) => {
        const cb = el("input", { type: "checkbox", class: "check", "aria-label": "Mark step done" });
        cb.checked = !!done[i]; li.classList.toggle("done", cb.checked);
        cb.onchange = () => { done[i] = cb.checked; li.classList.toggle("done", cb.checked); store.set("steps:" + id, JSON.stringify(done)); };
        li.appendChild(cb);
      });
    });
  }

  // ---------- filter: <input class="filter-box" data-target="#id"> filters .control children
  function filters() {
    document.querySelectorAll(".filter-box[data-target]").forEach((f) => {
      const box = document.querySelector(f.dataset.target);
      if (!box) return;
      f.addEventListener("input", () => {
        const q = f.value.toLowerCase();
        box.querySelectorAll(".control, tr").forEach((c) => { if (c.closest("thead")) return; c.style.display = !q || c.textContent.toLowerCase().includes(q) ? "" : "none"; });
      });
    });
  }

  document.addEventListener("DOMContentLoaded", () => {
    shell(); lightbox(); clips(); compares(); scrubbers(); hotspots(); tabs(); steps(); filters();
  });
  window.MSTGuide = { PAGES };
})();
