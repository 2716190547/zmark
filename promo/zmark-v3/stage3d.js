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

  function renderer(THREE, canvas) {
    const r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
    r.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    r.setSize(1280, 720, false);
    r.outputColorSpace = THREE.SRGBColorSpace;
    r.toneMapping = THREE.ACESFilmicToneMapping;
    r.toneMappingExposure = 1.05;
    r.shadowMap.enabled = true;
    r.shadowMap.type = THREE.PCFShadowMap;
    r.setClearColor(0x000000, 0);
    return r;
  }

  // Warm key from the upper left, cool rim from behind, soft fill.
  function paperLights(THREE, scene, { shadow = false } = {}) {
    scene.add(new THREE.HemisphereLight(0xf4efe6, 0x2a2b31, 0.85));
    const key = new THREE.DirectionalLight(0xffe6c4, 2.4);
    key.position.set(-4, 5, 6);
    if (shadow) {
      key.castShadow = true;
      key.shadow.mapSize.set(2048, 2048);
      key.shadow.radius = 6;
      key.shadow.bias = -0.0004;
      Object.assign(key.shadow.camera, { left: -6, right: 6, top: 6, bottom: -6, near: 0.5, far: 30 });
    }
    scene.add(key);
    const rim = new THREE.DirectionalLight(0xbcd2ff, 1.3);
    rim.position.set(3, 2, -6);
    scene.add(rim);
    return key;
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

  // Draws on every seek inside [start, end); outside it the canvas is left alone.
  function onSeek(start, end, draw) {
    window.addEventListener('hf-seek', (e) => {
      const t = e.detail.time;
      if (t >= start - 0.001 && t < end + 0.001) draw(t - start);
    });
  }

  window.Stage3D = { ready, build, renderer, paperLights, labelTexture, onSeek, clamp, seg, lerp, ease };
})();
