export const imageFormats = ["png", "jpg", "jpeg", "jfif", "webp"] as const;

export type ImageFormat = (typeof imageFormats)[number];
