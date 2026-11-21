const timeline = gsap.timeline({ paused: true, defaults: { ease: "power3.out" } });
window.__timelines ??= {};
window.__timelines["zmark-promo"] = timeline;

timeline
  .from(".opening-brand", { y: 10, opacity: 0.6, duration: 0.65 }, 0.1)
  .from(".opening-rule", { scaleX: 0, duration: 1.2 }, 0.15)
  .from(".reveal > span", { yPercent: 116, duration: 0.92, stagger: 0.24 }, 0.35)
  .from(".opening-copy p", { y: 20, opacity: 0, duration: 0.65 }, 1.08)
  .fromTo(".draft", { x: 230, y: 145, rotation: -22, opacity: 0 },
    { x: 0, y: 0, rotation: -10, opacity: 1, duration: 1.4 }, 0.32)
  .fromTo(".proof", { x: 245, y: 185, rotation: 18, opacity: 0 },
    { x: 0, y: 0, rotation: 7, opacity: 1, duration: 1.45 }, 0.64)
  .from(".draft-line i", { scaleX: 0, duration: 0.9 }, 1.22)
  .to(".opening-art", { x: -152, y: -22, scale: 1.07, duration: 1.12, ease: "power2.inOut" }, 4.09)

  .fromTo("#split", { clipPath: "inset(0 0 0 100%)" },
    { clipPath: "inset(0 0 0 0%)", duration: 1.05, ease: "power3.inOut" }, 4.4)
  .from(".window", { x: 315, scale: 0.91, duration: 1.4 }, 4.55)
  .from(".split-copy > *", { y: 32, opacity: 0, stagger: 0.12, duration: 0.85 }, 5)
  .from(".split-guide", { opacity: 0, duration: 0.6 }, 5.8)
  .fromTo(".split-guide span", { scaleX: 0 }, { scaleX: 0.52, duration: 1.7, ease: "sine.inOut" }, 7.28)
  .to(".pane-source", { y: -109, duration: 1.7, ease: "sine.inOut" }, 7.28)
  .to(".pane-preview", { y: -231, duration: 1.7, ease: "sine.inOut" }, 7.31)
  .to(".pane-source", { y: -204.61, duration: 1.85, ease: "sine.inOut" }, 10.12)
  .to(".pane-preview", { y: -456.01, duration: 1.85, ease: "sine.inOut" }, 10.15)
  .to(".split-guide span", { scaleX: 1, duration: 1.85, ease: "sine.inOut" }, 10.12)
  .fromTo(".click-ring", { opacity: 0, scale: 0.58 },
    { opacity: 1, scale: 1, duration: 0.28 }, 14.02)
  .to(".click-ring", { opacity: 0, scale: 1.42, duration: 0.55 }, 14.3)

  .fromTo("#lightbox", { opacity: 0 }, { opacity: 1, duration: 0.72, ease: "sine.inOut" }, 14.5)
  .fromTo(".lightbox-card", { x: 484, y: -73, scale: 0.423, opacity: 0.15 },
    { x: 0, y: 0, scale: 1, opacity: 1, duration: 1.27, ease: "power3.inOut" }, 14.5)
  .from(".lightbox-copy > *", { y: 19, opacity: 0, duration: 0.78, stagger: 0.1 }, 15.15)
  .from(".lightbox-tools", { y: 15, opacity: 0, duration: 0.75 }, 15.6)
  .to(".diagram", { scale: 1.065, x: -24, duration: 2.1, ease: "sine.inOut" }, 18.35)
  .to(".lightbox-card", { scale: 0.965, duration: 1.08, ease: "power2.inOut" }, 22.39)

  .fromTo("#closing", { clipPath: "circle(0% at 50% 52%)" },
    { clipPath: "circle(75% at 50% 52%)", duration: 1.13, ease: "power3.inOut" }, 22.65)
  .from(".closing-orbit", { opacity: 0, scale: 0.72, stagger: 0.09, duration: 1.55 }, 23.15)
  .from(".closing-lockup > *", { y: 28, opacity: 0, stagger: 0.17, duration: 0.92 }, 23.43)
  .from(".closing-footer", { y: 12, opacity: 0, duration: 0.8 }, 24.34)
  .from(".footer-rule", { scaleX: 0, duration: 1.15 }, 24.38)
  .set({}, {}, 30);

timeline.seek(0);
