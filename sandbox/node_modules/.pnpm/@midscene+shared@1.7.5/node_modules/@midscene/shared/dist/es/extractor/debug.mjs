import { webExtractTextWithPosition } from "./index.mjs";
import { setExtractTextWithPositionOnWindow, setMidsceneVisibleRectOnWindow } from "./util.mjs";
console.log(webExtractTextWithPosition(document.body, true));
console.log(JSON.stringify(webExtractTextWithPosition(document.body, true)));
setExtractTextWithPositionOnWindow();
setMidsceneVisibleRectOnWindow();
