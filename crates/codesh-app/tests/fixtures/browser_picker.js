const check = (condition, message) => { if (!condition) throw new Error(message); };
const run = new Function('return (async () => {' + pickerBody + '})();');
const cancel = new Function(cancelBody);
const active = new Map();
const add = window.addEventListener.bind(window);
const remove = window.removeEventListener.bind(window);
window.addEventListener = (type, handler, options) => { active.set(handler, type); add(type, handler, options); };
window.removeEventListener = (type, handler, options) => { active.delete(handler); remove(type, handler, options); };
const move = element => element.dispatchEvent(new PointerEvent('pointermove', { bubbles: true }));
const click = element => element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
const clean = () => {
    check(active.size === 0, 'picker listeners leaked');
    check(!window.__codeshDomPicker, 'picker handle leaked');
};

document.body.innerHTML = '<p id="plain">Hello</p>';
let element = document.getElementById('plain');
let original = element.outerHTML;
let result = run();
move(element);
check(element.style.cursor === 'pointer', 'pointer cursor missing');
check(element.style.outlineColor === 'rgb(18, 171, 239)', 'theme accent missing');
click(element);
let selected = await result;
check(selected.html === original, 'copied HTML includes picker decoration: ' + selected.html);
check(element.outerHTML === original, 'original style attribute not restored');
check(selected.selector === '#plain' && selected.bounds.width > 0, 'selection metadata missing');
clean();

for (const css of [
    'outline-color: red !important; cursor: crosshair !important;',
    'outline: 1px dashed blue; outline-color: red !important; outline-offset: 9px !important;',
    'outline: var(--ring) !important;',
]) {
    element.style.cssText = css;
    const outline = element.style.getPropertyValue('outline');
    const color = element.style.getPropertyValue('outline-color');
    const priority = element.style.getPropertyPriority('outline-color');
    const cursor = element.style.cursor;
    result = run();
    move(element);
    element.style.backgroundColor = 'blue';
    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    check(await result === null, 'Escape must cancel without a selection');
    check(element.style.getPropertyValue('outline') === outline, 'outline shorthand changed');
    check(element.style.getPropertyValue('outline-color') === color, 'outline longhand changed');
    check(element.style.getPropertyPriority('outline-color') === priority, 'outline priority changed');
    check(element.style.cursor === cursor, 'cursor not restored');
    check(element.style.backgroundColor === 'blue', 'concurrent style erased');
    clean();
}

element.style.cssText = 'outline-color: red !important';
result = run();
move(element);
element.style.cursor = 'wait';
move(document.body);
check(element.style.cursor === 'wait', 'page cursor update overwritten');
check(element.style.outlineColor === 'red', 'previous hover target not restored');
cancel();
check(await result === null, 'explicit cancellation returned data');
clean();

element.style.cssText = 'outline: var(--ring) !important;';
element.style.outlineColor = 'green';
const expectedStyle = element.style.cssText;
element.style.cssText = 'outline: var(--ring) !important;';
result = run();
move(element);
element.style.outlineColor = 'green';
cancel();
check(await result === null, 'cancellation returned data');
check(element.style.cssText === expectedStyle, 'shorthand restoration overwrote a concurrent longhand');
clean();

result = run();
move(element);
const stale = window.__codeshDomPicker;
const replacement = run();
check(await result === null, 'previous picker not cancelled');
stale.cancel();
check(Boolean(window.__codeshDomPicker), 'stale handle removed the replacement');
click(element);
check((await replacement).text === 'Hello', 'replacement picker failed');
clean();

element.removeAttribute('style');
result = run().then(() => null, error => error.message);
move(element);
const bounds = element.getBoundingClientRect;
element.getBoundingClientRect = () => { throw new Error('extraction failed'); };
click(element);
element.getBoundingClientRect = bounds;
check(await result === 'extraction failed', 'extraction error did not reject');
check(!element.hasAttribute('style'), 'extraction failure left decoration');
clean();

result = run().then(() => null, error => error.message);
const badMove = new PointerEvent('pointermove', { bubbles: true });
badMove.composedPath = () => { throw new Error('hover failed'); };
element.dispatchEvent(badMove);
check(await result === 'hover failed', 'hover error did not reject');
clean();

const button = document.createElement('button');
button.textContent = 'Copy without activating';
document.body.append(button);
const mouseEvents = ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click'];
const pageEvents = [];
for (const type of mouseEvents) {
    button.addEventListener(type, () => {
        pageEvents.push(type);
        button.textContent = 'Activated';
    });
}
const press = () => mouseEvents.map(type => {
    const Constructor = type.startsWith('pointer') ? PointerEvent : MouseEvent;
    return button.dispatchEvent(new Constructor(type, { bubbles: true, cancelable: true }));
});
result = run();
move(button);
const allowed = press();
selected = await result;
check(pageEvents.length === 0, 'picking activated page mouse handlers: ' + pageEvents.join(', '));
check(allowed.every(value => !value), 'picking allowed default mouse actions');
check(selected.text === 'Copy without activating', 'page action changed the copied text');
clean();
press();
check(pageEvents.join() === mouseEvents.join(), 'mouse handlers not restored after copying');
pageEvents.length = 0;
result = run();
move(button);
cancel();
check(await result === null, 'cancellation returned data');
clean();
press();
check(pageEvents.join() === mouseEvents.join(), 'mouse handlers not restored after cancellation');
button.remove();

document.body.innerHTML = '<p id="unicode"></p>';
element = document.getElementById('unicode');
const prefix = '<p id="unicode">';
element.textContent = 'x'.repeat(499 - prefix.length) + '😀' + 'tail'.repeat(1500);
result = run();
move(element);
click(element);
selected = await result;
check(Array.from(selected.html).length === 500 && selected.html.endsWith('😀'), 'HTML split a Unicode character or exceeded its bound');
check(selected.htmlTruncated && !selected.textTruncated, 'HTML/text limits conflated');
const html = selected.html;
element.textContent = 'x'.repeat(999999) + '😀tail';
result = run();
click(element);
selected = await result;
check(Array.from(selected.text).length === 1000000 && selected.text.endsWith('😀'), 'text split a Unicode character or exceeded its bound');
check(selected.textTruncated, 'text truncation missing');
clean();
return JSON.stringify({ html, textTail: selected.text.slice(-3), passed: true });
