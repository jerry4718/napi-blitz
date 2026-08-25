import { random } from "colord";

export function randomColor() {
    const color = random();
    if (color.brightness() < 0.5) {
        return color.invert()
    }

    return color;
}