import day from "./assets/sky-day.avif";
import night from "./assets/sky-night.avif";

export type Scene = "day" | "night";

export const SCENES: Record<Scene, string> = { day, night };

const decoded = new Set<Scene>();

/** Whether this session has shown the sky already. */
export const seen = (scene: Scene) => decoded.has(scene);

/** Decodes a sky off the main thread, so it is shown whole or not at all. */
export function decode(scene: Scene): Promise<void> {
  if (decoded.has(scene)) return Promise.resolve();
  const image = new Image();
  image.src = SCENES[scene];
  return image.decode().then(() => {
    decoded.add(scene);
  });
}
