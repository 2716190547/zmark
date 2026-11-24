// Shared helpers for the three Three.js shots. Every frame is drawn from the
// seek time alone; nothing here runs on a clock or an animation loop.
(function () {
  const ready = new Promise((resolve) => {
    if (window.THREE) resolve(window.THREE);
    else window.addEventListener('three-ready', () => resolve(window.THREE), { once: true });
  });

  const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const seg = (t, a, b) => clamp((t - a) / (b - a));
  const lerp = (a, b, k) => a + (b - a) * k;
  const ease = {
    inOut2: (k) => (k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2),
    inOut3: (k) => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2),
    out3: (k) => 1 - Math.pow(1 - k, 3),
    out4: (k) => 1 - Math.pow(1 - k, 4),
    expoOut: (k) => (k >= 1 ? 1 : 1 - Math.pow(2, -10 * k)),
    in2: (k) => k * k,
    in3: (k) => k * k * k,
  };
  // Seeded hash in [0, 1): the same index always gives the same value.
  const hash = (i, salt = 0) => {
    const x = Math.sin(i * 127.1 + salt * 311.7) * 43758.5453;
    return x - Math.floor(x);
  };

  function onSeek(start, end, draw) {
    window.addEventListener('hf-seek', (e) => {
      const t = e.detail.time;
      if (t >= start - 0.001 && t < end + 0.001) draw(t - start);
    });
  }

  // Registers a scene's async setup. setup resolves to draw(localTime), which
  // is called on every seek inside [start, end) of the root film.
  function build(key, start, end, setup) {
    window.__hf = window.__hf || {};
    window.__hf.buildReady = window.__hf.buildReady || {};
    const promise = ready.then(async (THREE) => {
      await document.fonts.ready;
      const draw = await setup(THREE);
      onSeek(start, end, draw);
      return draw;
    });
    window.__hf.buildReady[key] = promise;
    return promise;
  }

  // Neutral tone mapping keeps the brand hues saturated; the studio room
  // environment gives the clear coat clean reflections instead of grey falloff.
  function renderer(THREE, canvas, world) {
    const r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
    r.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    r.setSize(1280, 720, false);
    r.outputColorSpace = THREE.SRGBColorSpace;
    r.toneMapping = THREE.NeutralToneMapping;
    r.toneMappingExposure = 1.0;
    r.shadowMap.enabled = true;
    r.shadowMap.type = THREE.PCFShadowMap;
    r.setClearColor(0x000000, 0);
    if (world) {
      const pmrem = new THREE.PMREMGenerator(r);
      world.environment = pmrem.fromScene(new window.THREE_EXTRA.RoomEnvironment(), 0.04).texture;
      world.environmentIntensity = 0.9;
    }
    return r;
  }

  // One warm key from the upper left and a cool rim from behind.
  function studioLights(THREE, world, { shadow = false, key = 1.6, rim = 0.8 } = {}) {
    const k = new THREE.DirectionalLight(0xfff3e4, key);
    k.position.set(-4, 5, 6);
    if (shadow) {
      k.castShadow = true;
      k.shadow.mapSize.set(2048, 2048);
      k.shadow.bias = -0.0004;
      Object.assign(k.shadow.camera, { left: -8, right: 8, top: 8, bottom: -8, near: 0.5, far: 40 });
    }
    world.add(k);
    const r = new THREE.DirectionalLight(0xd8e4ff, rim);
    r.position.set(3, 2, -6);
    world.add(r);
    return k;
  }

  // The app icon's gradient, #C24127 → #F49D0B, as a texture.
  function brandTexture(THREE) {
    const c = document.createElement('canvas');
    c.width = c.height = 256;
    const g = c.getContext('2d');
    const lg = g.createLinearGradient(0, 0, 256, 256);
    lg.addColorStop(0, '#c24127');
    lg.addColorStop(1, '#f49d0b');
    g.fillStyle = lg;
    g.fillRect(0, 0, 256, 256);
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }
  const BRAND = ['#c24127', '#d4572a', '#e0702a', '#ea8620', '#f49d0b'];

  // Glossy lacquer: a saturated base under a clear coat.
  function lacquer(THREE, color, extra = {}) {
    return new THREE.MeshPhysicalMaterial({ color, roughness: 0.32, metalness: 0, clearcoat: 1, clearcoatRoughness: 0.06, ...extra });
  }
  const porcelain = (THREE) => lacquer(THREE, 0xfbfaf7, { roughness: 0.4 });

  // Markdown marks built from rounded bars, so no font file is needed.
  function glyph(THREE, kind, material, depth = 0.36) {
    const { RoundedBoxGeometry } = window.THREE_EXTRA;
    const g = new THREE.Group();
    const bar = (w, h, x, y, rz) => {
      const m = new THREE.Mesh(new RoundedBoxGeometry(w, h, depth, 4, Math.min(w, h, depth) * 0.3), material);
      m.position.set(x, y, 0); m.rotation.z = rz; m.castShadow = true; g.add(m);
    };
    const T = 0.32;
    if (kind === '#') { bar(T, 2.4, -0.42, 0, -0.16); bar(T, 2.4, 0.5, 0, -0.16); bar(2.2, T, 0.06, 0.45, 0); bar(2.2, T, -0.06, -0.45, 0); }
    if (kind === '*') { for (let i = 0; i < 3; i += 1) bar(T, 2.0, 0, 0, i * Math.PI / 3); }
    if (kind === '>') { bar(1.5, T, 0.05, 0.42, -0.62); bar(1.5, T, 0.05, -0.42, 0.62); }
    if (kind === '-') { bar(1.6, T, 0, 0, 0); }
    if (kind === '[]') { [-1, 1].forEach((s) => { bar(T, 2.2, s * 0.9, 0, 0); bar(0.6, T, s * 0.68, 0.94, 0); bar(0.6, T, s * 0.68, -0.94, 0); }); }
    if (kind === '`') { [-0.55, 0, 0.55].forEach((x) => bar(T * 0.9, 0.9, x, 0.4, -0.35)); }
    return g;
  }

  // A label drawn with the film's own font into a texture.
  function labelTexture(THREE, text, { w = 512, h = 256, bg = '#ffffff', ink = '#2c2c2c', border = '#9370db', size = 92, shape = 'rect', weight = 400 } = {}) {
    const c = document.createElement('canvas');
    c.width = w; c.height = h;
    const g = c.getContext('2d');
    g.fillStyle = bg;
    g.fillRect(0, 0, w, h);
    g.strokeStyle = border;
    g.lineWidth = 10;
    if (shape === 'rect') g.strokeRect(5, 5, w - 10, h - 10);
    g.fillStyle = ink;
    g.font = `${weight} ${size}px "Promo Sans", sans-serif`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillText(text, w / 2, h / 2 + size * 0.04);
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    tex.anisotropy = 8;
    return tex;
  }

  // World units per screen pixel on the z = 0 plane for a camera at distance d.
  const perPx = (THREE, fov, d) => (2 * Math.tan(THREE.MathUtils.degToRad(fov / 2)) * d) / 720;

  window.Stage3D = { ready, build, renderer, studioLights, brandTexture, BRAND, lacquer, porcelain, glyph, labelTexture, onSeek, perPx, clamp, seg, lerp, ease, hash };
})();
