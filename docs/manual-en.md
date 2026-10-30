# 7rrek for Beginners

7rrek (حرّك) is a drawing and animation app. This guide is for people who have never used an animation app. By the end you will have made your first animation and exported it as a video.

> Button and menu names are written as they appear in the English interface.

---

## 1. Five ideas you need first

**Frame.** An animation is a series of still pictures shown quickly one after another, so the eye sees movement. Each picture is a frame.

**Frames per second (FPS).** How many pictures are shown each second. 24 is the usual number for cartoons. A two-second animation at 24 FPS needs 48 frames.

**Keyframe.** A frame where you draw an important pose: the ball on the ground, then the ball at the top. A keyframe's drawing stays on screen in the frames after it until the next keyframe.

**Motion Tween.** Instead of drawing every frame by hand, you draw two poses and the app draws the frames in between: it moves, turns and scales the shape, and blends its colour.

**Easing.** Real things don't move at a constant speed: a car starts slowly, speeds up, then slows down to stop. Easing gives a tween that feel, so movement looks natural instead of mechanical.

---

## 2. The screen

- **The canvas** in the middle: the white page is what will appear in the video.
- **The tools** on the side of the screen: Selection, shapes, Pen, Paintbrush and more.
- **Layers** in the side panel.
- **The Timeline** at the bottom, the heart of the app:
  - Each **row** in the timeline is one of your layers.
  - The numbers along the top are frame numbers.
  - The vertical **red line** marks the frame you are looking at. Click any number to go there.
  - A **dot** in a row is a keyframe, and the **band** after it means the drawing holds.
  - An **arrow** between two dots means there is a tween between them.
  - At the top: the playback buttons, the **FPS** box, and the **Frames** box for the animation's length.

Every animation command is also in the **Animation** menu at the top of the screen.

---

## 3. Your first animation: a bouncing ball

### Step 1: a new document
Choose **File › New…** to create a document. In the timeline, type 24 in the **FPS** box and 24 in the **Frames** box. That is one second of animation.

### Step 2: draw the ball
Pick the **Ellipse** tool and draw a small circle at the bottom left of the page, then give it a colour.

### Step 3: the first keyframe
Make sure the red line is on frame 1, then click **Keyframe**. A dot appears in the layer's row.

### Step 4: the second keyframe
Click frame 24 in the timeline, then **Keyframe** again. With the **Selection** tool, drag the ball to the bottom right of the page.

### Step 5: make it move
Go back to frame 1 and click **Motion Tween**. An arrow appears between the two dots. Click **Play**: the ball travels from left to right.

### Step 6: make it bounce
The ball now slides along a straight line. To make it jump:
1. Go to frame 12 (the middle) and click **Keyframe**.
2. Drag the ball up to the top of the page.
3. Click **Motion Tween** on frame 12 too, so the second half moves as well.

### Step 7: easing
Select frame 1, click **Easing** and choose **Ease Out**: the ball leaves fast and slows down as it rises. On frame 12 choose **Ease In**: it starts slowly at the top and speeds up as it falls. Play it and compare.

Under the layer's row you will see the **Timing Chart**: a small tick for each frame between the two dots. Where the ticks are close together the movement is slow; where they are far apart it is fast.

### Step 8: save
**File › Save**. 7rrek files are saved with the `.7rrek` extension.

---

## 4. Drawing frame by frame

Tweens suit things that move and turn. Water, fire and facial expressions are usually drawn frame by frame:

1. Draw the first drawing on frame 1.
2. Go to frame 3 and click **Blank Keyframe**: an empty page for a new drawing.
3. Turn on **Onion Skin**: you see the previous drawing faintly under yours, so you can draw the next step while seeing where it was.
4. Repeat. Leaving one frame between drawings is called "animating on twos", a common cartoon technique that halves the drawing.

**Light Table** shows the other rows faded while you draw on one. To remove a keyframe, use **Clear Keyframe**.

---

## 5. More than one layer

Put each thing that moves on its own on its own layer: the background on one layer, the character on another, the ball on a third. Each layer is a row in the timeline with its own keyframes, so the ball can move while the background stays still.

A **peg** is a handle you hang one or more layers on, to move them together. Select the layer and click **Add Peg**, then move the peg and click **Peg Key** at each pose. Pegs nest: the hand hangs on the arm and the arm on the body, so when the body turns the arm and hand follow.

---

## 6. The camera

**Camera Key** keeps what the camera sees at this frame: its position, zoom and rotation. Put one key at the start and another at the end with a closer zoom, and the camera moves in on the scene in the video.

---

## 7. Sound

**Import Sound…** places a WAV, MP3 or Ogg file at the current frame. Its waveform shows in the timeline, and it plays with the animation and in the video.

**Lip Sync**: if you have drawn a character's mouth in different shapes (open, closed, round…), 7rrek picks the mouth shape that matches the sound for each frame.

---

## 8. Characters

**Animation › Load Character…** adds a ready-made character, with its bones and pegs, to the scene. Once you have built a character, **Save Character…** keeps it for other scenes.

A character drawn in a drawing app can be turned into a character ready to animate, then given ready-made actions such as walking and standing idle. Today the easiest way to do this is to ask an AI assistant. Turn it on from **Help › AI Agents…**, then ask for example: "Load the character from this file and make it walk for two seconds."

---

## 9. Exporting

- **Export Video…**: an MP4 file to share anywhere, or WebM. Sound is included.
- **Export GIF…**: a short animated picture, without sound.

Play the animation from the start one last time before you export.

---

## 10. Tips for getting started

- Start with short animations: one or two seconds.
- Draw the important poses first, then fill in between them.
- Almost nothing in nature moves at a constant speed: use easing nearly every time.
- If the movement feels too fast, move the keyframes further apart. If it feels slow, bring them closer.
- Save often. To take back the last step: **Undo**.

---

## Glossary

| Word | Meaning |
|---|---|
| Frame | One picture of the animation |
| FPS | Frames per second |
| Keyframe | A frame holding a pose you draw yourself |
| Blank Keyframe | A keyframe that starts with an empty page |
| Motion Tween | The app draws the frames between two keyframes |
| Easing | How the movement speeds up and slows down |
| Timing Chart | Ticks that show where the movement slows and speeds up |
| Onion Skin | Seeing the previous and next frames faintly |
| Peg | A handle that moves one or more layers together |
| On twos | A new drawing every two frames instead of every frame |
