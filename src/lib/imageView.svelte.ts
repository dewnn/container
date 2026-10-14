// Zoom, pan and before/after comparison state for the still-image viewer.
export class ImageView {
  compare = $state(50);
  viewport: HTMLElement | null = $state(null);
  zoom = $state(1);
  baseScale = $state(1);
  panX = $state(0);
  panY = $state(0);
  dragging = $state(false);
  initialized = $state(false);

  constructor(private readonly media: () => { width: number | null; height: number | null } | null) {}

  /** Forget the view of the previous image. */
  clear() {
    this.compare = 50;
    this.zoom = 1;
    this.baseScale = 1;
    this.panX = 0;
    this.panY = 0;
    this.dragging = false;
    this.initialized = false;
  }

  startCompare = (event: PointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const handle = event.currentTarget as HTMLElement;
    const comparison = handle.closest<HTMLElement>(".image-compare");
    if (!comparison) return;
    const rectangle = comparison.getBoundingClientRect();
    const pointerId = event.pointerId;
    const update = (clientX: number) => {
      this.compare = Math.max(0, Math.min(100, (clientX - rectangle.left) / rectangle.width * 100));
    };
    update(event.clientX);
    const move = (moveEvent: PointerEvent) => {
      if (moveEvent.pointerId === pointerId) update(moveEvent.clientX);
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  };

  compareKey = (event: KeyboardEvent) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    this.compare = Math.max(0, Math.min(100, this.compare + (event.key === "ArrowRight" ? 2 : -2)));
  };

  resetView = () => {
    this.zoom = 1;
    this.panX = 0;
    this.panY = 0;
    const rectangle = this.viewport?.getBoundingClientRect();
    const media = this.media();
    if (!rectangle || !media?.width || !media.height) {
      this.baseScale = 1;
      return;
    }
    const fittedPixelScale = Math.min(rectangle.width / media.width, rectangle.height / media.height);
    this.baseScale = fittedPixelScale > 1 ? 1 / fittedPixelScale : 1;
  };

  initialize = () => {
    if (this.initialized) return;
    this.initialized = true;
    this.resetView();
  };

  transform = () => `translate3d(${this.panX}px, ${this.panY}px, 0) scale(${this.baseScale * this.zoom})`;

  setZoom = (nextZoom: number, clientX?: number, clientY?: number) => {
    const next = Math.max(0.1, Math.min(12, nextZoom));
    if (Math.abs(next - this.zoom) < 0.0001) return;
    const rectangle = this.viewport?.getBoundingClientRect();
    if (rectangle && clientX != null && clientY != null) {
      const anchorX = clientX - rectangle.left - rectangle.width / 2;
      const anchorY = clientY - rectangle.top - rectangle.height / 2;
      const ratio = next / this.zoom;
      this.panX = anchorX - (anchorX - this.panX) * ratio;
      this.panY = anchorY - (anchorY - this.panY) * ratio;
    }
    this.zoom = next;
  };

  wheel = (event: WheelEvent) => {
    event.preventDefault();
    const factor = Math.exp(-event.deltaY * 0.0015);
    this.setZoom(this.zoom * factor, event.clientX, event.clientY);
  };

  startPan = (event: PointerEvent) => {
    if (event.button !== 0) return;
    const target = event.target as Element;
    if (target.closest(".compare-handle, .image-view-controls")) return;
    event.preventDefault();
    const startX = event.clientX;
    const startY = event.clientY;
    const originX = this.panX;
    const originY = this.panY;
    this.dragging = true;
    const move = (moveEvent: PointerEvent) => {
      this.panX = originX + moveEvent.clientX - startX;
      this.panY = originY + moveEvent.clientY - startY;
    };
    const stop = () => {
      this.dragging = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  };
}
