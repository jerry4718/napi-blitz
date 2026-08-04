import {useWindow} from "./useWindow.ts";

type DragInfo<Target> = { base: Target, start: PointerEvent, end: boolean };
type DragUpdate<Target> = (current: PointerEvent, drag: DragInfo<Target>) => unknown;

export function useDragUpdate<Target> (update: DragUpdate<Target>) {
    const window = useWindow();

    return function (this: any, start: PointerEvent) {
        const base = start.currentTarget as Target;

        update.call(this, start, { start, end: false, base });

        function pointermove (this: any, current: PointerEvent) {
            update.call(this, current, { start, end: false, base });
        }

        function pointerup (this: any, current: PointerEvent) {
            window.removeEventListener('pointermove', pointermove);
            window.removeEventListener('pointerup', pointerup);
            update.call(this, current, { start, end: true, base });
        }

        window.addEventListener('pointermove', pointermove);
        window.addEventListener('pointerup', pointerup);
    };
}
