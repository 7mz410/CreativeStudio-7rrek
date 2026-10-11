# ShibShib — ملف المشروع (اقرأه أولاً)

آخر تحديث: 2026-10-10 (بعد جلسة البرينستورم). هذا الملف هو نقطة البداية لأي جلسة جديدة. اقرأه بدل إعادة استكشاف المشروع، وحدّثه في نهاية كل جلسة.

## 1. الفكرة

**شبشب (ShibShib)**: حزمة برامج تصميم مجانية ومفتوحة المصدر، بديلة لبرامج Adobe، بواجهة عربية وإنجليزية. صاحب المشروع حمزة أبو عياش، مصمم يعمل على برامج Adobe منذ 2004. الهدف لاحقاً ربطها بموقع the-247.com.

| البرنامج | بديل | الأساس | الحالة |
|---|---|---|---|
| **rsm** (رسم) | Illustrator | fork من VectorCraft | ✅ شغّال، والواجهة العربية جارية |
| tlween (تلوين) | Photoshop | PhotoCraft | لاحقاً |
| trteeb (ترتيب) | InDesign | DesignCraft (لسا upstream بيتطور) | لاحقاً |
| tsweer (تصوير) | Premiere | FilmCraft | لاحقاً |
| effectat (تأثيرات) | After Effects | EffectCraft | لاحقاً |
| 7rrek (حرّك) | Animate/Flash | **نبنيه فوق rsm**: timeline، tweens، frame by frame | بعد rsm مباشرة |
| sharek (شارك) | Figma | Penpot على الأغلب، ويحتاج سيرفر | لاحقاً |
| ttshat (تتشات) | Lightroom | LightCraft (من نفس عائلة ArtCraft) | سهل، بعد 7rrek |
| mzika (مزيكا) | FL Studio | LMMS (GPL) | لاحقاً: مقامات بربع تون، آلات وإيقاعات شرقية |
| sf7aat (صفحات) | Acrobat | PdfCraft | لاحقاً: بعد effectat |
| aswat (أصوات) | Audition | Audacity/Tenacity (GPL) أو SoundCraft، يحتاج فحصاً | لاحقاً: تحويل الكلام العربي لنص |

اللوغوهات (SVG) موجودة في `docs/shibshib/suite/`، وأصلها في `~/Documents/ShibShib/logos` (الشعار الرئيسي: `shibshib_main.svg`). النصوص فيها outlines؛ النسخ القابلة للتعديل (نص حي بخط Bricolage Grotesque ExtraBold) في `~/Documents/ShibShib/logos-editable/` (محلية، مش على GitHub). بعد أي تعديل على اللوغوهات: انسخها لـ `suite/`، وأعد توليد `rsm-white.svg`، وأضف أي ملف جديد لـ `ASSETS.md`. أيقونة البرنامج تأتي من `mark-rsm-white.svg` (عبر `packaging/icons.sh`).

## 2. الأماكن المهمة

- **الفولدر الأم:** `~/Documents/ShibShib/`، وفيه فولدر لكل برنامج (`rsm/`، وبعدين `ttshat/` و`mzika/`…)، وفولدر `logos/`.
  - الفولدر الأم نفسه ريبو الموقع **https://github.com/7mz410/shibshib.art**، وهو public لأن GitHub Pages يتطلب ذلك في الحساب المجاني.
  - كل برنامج ريبو مستقل، ومستثنى من ريبو الموقع عبر `.gitignore`.
- **الدومين:** `shibshib.art`، مشترى من Spaceship في 2026-10-10.
  - `shibshib.art`: صفحة الحزمة (ملف `index.html` في الفولدر الأم).
  - `rsm.shibshib.art`: نسخة الويب من rsm.
  - لاحقاً: subdomain لكل برنامج، ثم نقل الاستضافة لسيرفر يعمل 24/7 من أجل sharek.

- **الريبو:** https://github.com/7mz410/ShibShib-rsm، وهو fork لـ `storytold/vectorcraft`.
  - `origin` هو ريبونا، و`upstream` هو ArtCraft.
- **النسخة المحلية:** `~/Documents/ShibShib/rsm`
- **الويب:** https://rsm.shibshib.art/ من فرع `gh-pages`، ويُرفع يدوياً (انظر §6).
- **ملفاتنا داخل الريبو كلها في `shibshib/`:**
  - `PROJECT.md`: هذا الملف.
  - `CHANGES.md`: ما غيّرناه عن upstream، وهو مطلوب لرخصة Apache.
  - `rebrand.py`: يطبّق الاسم والشعار. شغّله بعد كل merge من upstream.
  - `ar-work/`: ملفات الترجمة على دفعات. `assemble.py` يبني منها `crates/ui-egui/src/i18n/ar.tsv`، و`check.py` يفحص كل دفعة.
- **الكود العربي:**
  - `crates/ui-egui/src/i18n/ar.tsv`: الترجمة.
  - `crates/ui-egui/src/i18n/bidi.rs`: ترتيب الكلمات من اليمين لليسار.
  - `plural_arabic` في `i18n/mod.rs`: قواعد الجمع العربية.
  - خط IBM Plex Sans Arabic في `assets/fonts`.
- **المشروع القديم (React):** محذوف. لا ترجع له.

## 3. القواعد الثابتة (مهمة)

1. **الرخصة والـ credits:**
   - نحافظ على `LICENSE-MIT` و`LICENSE-APACHE` و`NOTICE`، وعلى حقوق "ArtCraft Team and the VectorCraft contributors".
   - شكر VectorCraft موجود في الـ README وشاشة About.
   - ممنوع استخدام شعارات ArtCraft أو اسمهم كهوية لنا.
2. **تغيير الاسم الآن للنصوص الظاهرة فقط** (استثناء: امتداد الملف صار `.rsm` لكل برنامج امتداده باسمه، وأيقونة ملف بلون البرنامج من `~/Documents/ShibShib/file icons/`). أسماء الـ crates وصيغة `.vectorcraft` تبقى كما هي، حتى يبقى الـ merge من upstream سهلاً. حمزة قرر أن نغيّر كل شيء **لاحقاً**، لا الآن.
3. **الـ commits:**
   - باسم `Hamza Abu Ayyash <hamza.abu3ayash@gmail.com>` فقط.
   - **ممنوع إضافة Co-Authored-By Claude**، لأن بعض الخدمات ترفضه.
4. **التوكنات:** لا نخزّن أي توكن في ملف. `gh` مسجّل دخول على جهاز حمزة.
5. **الخطوط:** نستعمل Google Fonts بدل Adobe Fonts.
6. **الكتب المرجعية** (PDF وEPUB) محمية بحقوق نشر، ولا تُرفع للريبو أبداً.
7. **المساحة على الديسك (2026-10-10، امتلأ مرة):** ملفات البناء (`target/`) تصل 10–30 GB لكل برنامج. نحذف أولاً بأول: `target/` البرنامج اللي خلصنا منه (أو `cargo clean`)، ومجلدات `target/agent-*`، ونفحص `df -h ~` قبل أي بناء كبير. تبقى `dist/` (برامج الماك) والكود.
8. **الفحوصات قبل كل commit:**
   - `cargo xtask assets`: كل ملف asset له سطر في `ASSETS.md`.
   - `cargo xtask brands`: لا أسماء شركات في نص الواجهة.
9. **التواصل:** حمزة يكتب بالعربي العامي، والرد يكون بنفس اللغة، مختصراً ومباشراً.

## 4. وين كنا ← وين صرنا

1. بدأنا بمحرر React كتبناه بأنفسنا (VectorForge ثم ShibShib)، ثم أوقفناه.
2. اكتشفنا ArtCraft (برامج بلغة Rust، رخصتها MIT/Apache) وقررنا عمل fork.
3. **rsm:**
   - تم: fork، وتغيير الاسم، والأيقونة، والـ credits، ونسخة Mac (`dist/ShibShib rsm.app`)، ونسخة ويب.
   - تم: اللغة العربية بالكامل، 3652 جملة. الحروف تتصل صح، وترتيب الكلمات صحيح.
4. **آخر نقطة (2026-10-10):**
   - تصلّح اختبار قائمة اللغات ("English" يبقى بالإنجليزية).
   - الأرقام مثل 1920×1080 صارت صحيحة في النص العربي.
   - انشال زر Discord ورابط ArtCraft من الواجهة.
   - الويب محدّث بالواجهة العربية الكاملة.
   - صارت عندنا 4 سكيلز خاصة في `.claude/skills/` (انظر §8).
   - الاختبار الوحيد الفاشل هو `save_a_copy_and_template_suggest_their_names`، ويفشل في upstream نفسه.
   - كل الاختبارات تمرّ (828).
   - **دمج upstream (2026-10-10):** 96 commit، منها خطوط عربية للنصوص بالويب (PR #754)، ومكتبات (Libraries)، وPie للشكل البيضاوي، وUnderline/Strikethrough، والأوكرانية. ترجمنا 53 جملة جديدة (دفعة 15).
   - الموقع صار عربي وإنجليزي بزر تبديل، واللوغوهات outlines.

## 4.5 قرارات البرينستورم (2026-10-10)

- **الخطّاط (لوحة الخط العربي داخل rsm)** هي الإضافة الحقيقية لشبشب، وتأتي بعد RTL مباشرة. فيها:
  - كشيدة تمتد بالسحب، وبدائل الحروف، وتحريك التشكيل.
  - التركيب على طريقة الثلث والديواني، وقوالب تكوينات.
  - التنفيذ clean-room: لا نفكك برامج مغلقة مثل كلك أو Tasmeem. الأساس هو harfrust الموجود، وخطوط Google المفتوحة (Amiri وAref Ruqaa وReem Kufi وScheherazade New).
- **الاستقلال (hard fork):** حمزة يريد لاحقاً fork مستقلاً، مع نسب الأصل في قائمة منفصلة.
  - **إلزامي:** ملفات الرخصة، وسطور الحقوق، ومحتوى NOTICE.
  - **اختياري:** الشكر في README أو About، ولافتة "forked from" (فصلها عبر دعم GitHub).
  - **الخطة:** `CREDITS.md`، وسطر واحد في README وAbout.
  - **التوقيت:** بعد أن تتعمق تعديلاتنا (RTL، الخطّاط، الأسماء الداخلية).
  - البرامج المبنية على GPL تبقى GPL.
- الحزمة صارت 10 برامج. ترتيب الأولوية:
  1. rsm مع RTL والخطّاط.
  2. 7rrek.
  3. ttshat.
  4. mzika.
  5. الباقي.

## 5. شو بدنا نعمل (بالترتيب)

**قرار 2026-10-10 (مساءً): كل شغل الترجمة العربية مؤجّل للأسبوع الجاي.** هلأ بس: fork، هوية، أيقونة، كريدتس، ويب. بعدين جولة ترجمة واحدة لكل الباقي (effectat أولاً، وبعدها sf7aat وغيره).

قرار 2026-10-10: نبني السويت كلها أولاً، ثم نضيف على كل برنامج ميزات عربية خاصة. الـ RTL أولاً لأنه ينتقل لكل برامج عيلة ArtCraft (نفس egui).

1. ~~إصلاح الاختبار ورفع الويب~~ ✅، ~~دمج upstream~~ ✅ (كرّره دورياً بـ `shibshib-sync`)، ~~إزالة بقايا ArtCraft~~ ✅، ~~امتداد `.rsm` وأيقونة الملف~~ ✅
2. ~~**RTL**~~ ✅ (2026-10-10): مفتاح واحد في egui المعدّل (`vendor/egui`، `egui::set_rtl`) يقلب الواجهة كلها (اللوحات، الصفوف، Grid، الأعمدة، القوائم، النوافذ). باقي تفاصيل مرسومة بإحداثيات ثابتة: صفوف الطبقات، تبويبات المستندات، أزرار أسفل اللوحات. التفاصيل في `shibshib/RTL.md`.
3. ~~**الـ agents كميزة**~~ ✅ (2026-10-10): Help › AI Agents… (وكلاء الذكاء الاصطناعي): تفعيل قناة التحكم (تبدأ مع البرنامج بعدها)، وأزرار ربط Claude Desktop وClaude Code وGemini CLI، ونص إعداد للباقي. باقي: إشارة اتصال بشريط الحالة. أيقونة الديسكتوب "شبشب رسم + Agents" ما عاد لها لزوم.
4. **برامج عيلة ArtCraft** (عبر `shibshib-fork-app`، كل واحد ياخذ الهوية والعربي والـ RTL والـ MCP): ttshat (LightCraft) ← tlween (PhotoCraft) ← tsweer (FilmCraft) ← effectat (EffectCraft) ← **sf7aat (صفحات)، بديل Acrobat من PdfCraft** (اللوغو `logos/sf7aat.svg`، الأيقونة `file icons/sf7aat.svg`، الاسم أبيض `file icons/sf7aat-name-white.svg`) ← trteeb (DesignCraft، عندما يجهز).
   - **ttshat (جاري، 2026-10-10):** fork في https://github.com/7mz410/ShibShib-ttshat، والنسخة المحلية `~/Documents/ShibShib/ttshat`. تم: الهوية (`shibshib/rebrand.py`)، الأيقونة، الكريدتس، حذف `docs/brand`، الـ RTL (`vendor/egui` منسوخ من rsm)، خط IBM Plex Sans Arabic، `bidi.rs`. باقي: الترجمة العربية (1878 جملة + 230 صيغة) في `shibshib/ar-work/`، بعدها نضيف `Ar` لجدول اللغات في `i18n.rs`. نظام الترجمة هون JSON (`crates/ui-egui/locales/<code>.json` و`<code>-formats.json`)، مش TSV متل rsm. 5 اختبارات `tests_model_setup` بتفشل بالأصل نفسه.
   - **ttshat ✅ (2026-10-10):** العربي كامل (2108 جملة، `shibshib/ar-work/`، صيغ format! بـ `{:.0}` لإخفاء لاحقة الجمع الإنجليزية)، وصار بقائمة اللغات. على الويب: https://ttshat.shibshib.art. باقي: screenshot بالعربي ومراجعة حمزة للمصطلحات، وفحص الواجهة بالـ RTL.
   - **tlween ✅ (2026-10-10):** fork من PhotoCraft (https://github.com/7mz410/ShibShib-tlween، محلياً `~/Documents/ShibShib/tlween`). تم: الهوية والأيقونة والكريدتس، حذف زر Discord ولينك ArtCraft، الـ RTL (`vendor/egui`)، خط IBM Plex Sans Arabic، `bidi.rs`، والعربي كامل (2766 جملة، `shibshib/ar-work/`، جمع عربي بست صيغ وكل صيغة فيها `{n}`). كل تعديلات ملفات upstream مسجّلة في `RTL_RULES` بـ `rebrand.py`. الاختبارات 1580/1580. الويب: https://tlween.shibshib.art. باقي: مراجعة حمزة للمصطلحات، وفحص الواجهة بالـ RTL بالتفصيل.
   - **tsweer ✅ (2026-10-10):** fork من FilmCraft (https://github.com/7mz410/ShibShib-tsweer، محلياً `~/Documents/ShibShib/tsweer`). تم: الهوية والأيقونة والكريدتس (ملف `.attribution` لكل asset، يفحصه `cargo xtask assets`)، شعار tsweer مكان شعار ArtCraft، حذف زر Discord، الـ RTL، خط IBM Plex Sans Arabic، والعربي كامل (3495 جملة). اللوحات بتصوير مبنية بتخطيط خاص، فما بتنقلب جهاتها لحالها (باقي). `rebrand.py` بيغيّر الاسم بمفاتيح الترجمة بس إذا الكود غيّره (بعض الـ crates برّا الواجهة لسا فيها FilmCraft). الويب: https://tsweer.shibshib.art.
   - **effectat (2026-10-10):** fork من EffectCraft (https://github.com/7mz410/ShibShib-effectat، محلياً `~/Documents/ShibShib/effectat`). تم: الهوية والأيقونة (مكان علامة ArtCraft بشريط الأدوات وAbout)، روابط Help، الكريدتس. الويب: https://effectat.shibshib.art. **العربي مؤجّل:** ترجمات EffectCraft مكتوبة جوّا الكود (`crates/ui-egui/src/i18n.rs` و`i18n/ui.rs`، 6500 سطر)، لازم أولاً ننقلها لملف TSV منفصل بطريقة ما تتعارض مع upstream. 10 اختبارات `pointer_wrap` بتفشل بالأصل نفسه. ملف المشروع (XML) فيه اسم EffectCraft، ممنوع نغيّره (صيغة محفوظة).
   - **sf7aat (صفحات) (2026-10-10):** fork من PdfCraft (https://github.com/7mz410/ShibShib-sf7aat، محلياً `~/Documents/ShibShib/sf7aat`). تم: الهوية والأيقونة، شعار sf7aat مكان شعار وعلامة ArtCraft، روابط Help والشاشة الرئيسية، فحص التحديثات صار يسأل ريبونا. الأصول هون مسجّلة بـ `ATTRIBUTION.toml` مع sha256 لكل ملف (بعد `rebrand.py` لازم نحدّث الـ hashes ونشغّل `cargo xtask assets --write`). الاختبارات كلها تمر. الويب: https://sf7aat.shibshib.art. العربي مع جولة الترجمة الأسبوع الجاي.
   - **ملاحظة لكل rebrand.py:** قاعدة الاستبدال بنص فاضي لازم تتطبّق كل مرة (`if not b or b not in out`)، انصلحت بالثلاثة.
   - **الدومينات:** حمزة عمل subdomain لكل برنامج (CNAME إلى 7mz410.github.io). كل برنامج بنرفع نسخة الويب تبعه على فرع `gh-pages` بريبوه مع ملف CNAME.
5. **برامج من عيلة ثانية** (C++/Qt، GPL): mzika (LMMS) ← aswat (Audacity/Tenacity) ← sharek (Penpot، يحتاج سيرفر).
6. **7rrek:** يُبنى فوق rsm (timeline، keyframes، onion skin، tweens، تصدير فيديو/GIF/Lottie).
7. **MCP موحّد للسويت** (`shibshib mcp`) يعرف كل البرامج المفتوحة.
8. **مرحلة الإضافات العربية:** الخطّاط في rsm (§4.5)، مقامات بربع تون في mzika، تحويل الكلام العربي لنص في aswat…
9. لاحقاً: PR للترجمة العربية على upstream، تغيير الأسماء الداخلية (hard fork)، ربط the-247.com.

## 6. أوامر متكررة

```sh
cd ~/Documents/ShibShib/rsm
# مزامنة مع upstream
git fetch upstream && git merge upstream/main && python3 shibshib/rebrand.py
# الاختبارات
cargo test -q -p vectorcraft-ui-egui && cargo xtask assets && cargo xtask brands
# نسخة Mac
cargo xtask bundle && mv dist/VectorCraft.app "dist/ShibShib rsm.app"
# نسخة الويب (rustup مثبّت عبر brew وهو keg-only)
cd apps/vectorcraft-web && PATH="/opt/homebrew/opt/rustup/bin:$PATH" trunk build --release
# رفع الويب: انسخ dist/web إلى مجلد git على فرع gh-pages، وأضف .nojekyll، ثم push -f إلى gh-pages
# الترجمة
python3 shibshib/ar-work/check.py shibshib/ar-work NN   # فحص دفعة
python3 shibshib/ar-work/assemble.py                    # بناء ar.tsv
```

اختبار الواجهة في المتصفح: Playwright (`playwright-core` مع Google Chrome)، والـ locale على `ar` حتى يفتح البرنامج بالعربي.

## 7. الأدوار (نحن فريق)

| | حمزة (المصمم وصاحب القرار) | Claude (المهندس) |
|---|---|---|
| الرؤية | يحدد البرامج والأولويات والهوية | يقترح الطريق التقني الأبسط والأسلم |
| التصميم | اللوغوهات والألوان، وتجربة الاستخدام كمصمم محترف | يطبّق الهوية في الكود والأيقونات |
| الكود | — | يكتب ويختبر ويرفع، ويحافظ على الرخص والـ credits |
| الاختبار | يجرّب البرنامج فعلياً ويبلّغ عن المشاكل | اختبارات آلية وscreenshots قبل كل تسليم |
| الترجمة | يراجع المصطلحات العربية | يترجم على دفعات مع فحص آلي |
| القرارات الصعبة | حذف، أو نشر، أو تغيير اتجاه، أو تكاليف (سيرفر ودومين) | يسأل قبل أي خطوة لا رجعة فيها |

**التسلسل في كل مهمة:**
1. حمزة يطلب.
2. Claude يفحص ويقترح (بدون تنفيذ إذا طُلب ذلك).
3. حمزة يوافق.
4. Claude ينفّذ ويختبر.
5. Claude يرفع ويعطي رابطاً أو ملفاً.
6. حمزة يجرّب ويعطي ملاحظات.
7. Claude يحدّث هذا الملف.

## 8. السكيلز

**الموجودة ونستعملها:**

| السكيل | متى |
|---|---|
| `code-review` | قبل أي merge كبير، أو بعد ميزة جديدة (مستوى high) |
| `simplify` | بعد كل ميزة، لتنظيف الكود وإزالة التكرار |
| `security-review` | قبل أي إصدار، وخاصة لاحقاً مع sharek والسيرفر |
| `run` | لتشغيل البرنامج والتأكد من التعديل فعلياً |
| `replica-brand` | فحص بقايا اسم أو شعار ArtCraft بعد كل دمج |
| `replica-test` | خطة اختبار كاملة للأدوات (click-through وPlaywright) |
| `replica-diff` | قياس التكافؤ مع Illustrator وما الذي ينقص |
| `ponytail` | أبسط حل يعمل، ومنع التعقيد |
| `caveman` | ردود قصيرة ومباشرة بدون حشو. حمزة بيفضّلها، شغّلها من أول الجلسة |
| `humanizer` | تحسين نصوص الـ README والإعلانات قبل النشر |
| `artifact-design` و`docs` | صفحات وتقارير نشاركها |
| `loop` / `schedule` | متابعة upstream بشكل دوري، مثلاً تقرير يومي بالجديد |
| `skill-creator` | لبناء السكيلز الخاصة بنا (تحت) |
| `fewer-permission-prompts` | تقليل طلبات الإذن للأوامر الآمنة المتكررة |

**سكيلزنا الخاصة (موجودة في `.claude/skills/` داخل الريبو، وتعمل عندما تبدأ الجلسة من مجلد الريبو):**
1. **`shibshib-sync`:** fetch، ثم merge من upstream، ثم `rebrand.py`، ثم الاختبارات، ثم assets وbrands، ثم تقرير.
2. **`shibshib-release`:** بناء Mac والويب، وscreenshots عربي وإنجليزي، ورفع gh-pages. فيه `scripts/release.sh` و`scripts/screenshot.mjs`.
3. **`shibshib-arabic`:** إضافة الجمل الجديدة للترجمة (`scripts/new_strings.py`)، مع قاموس مصطلحات ثابت.
4. **`shibshib-fork-app`:** وصفة fork برنامج جديد (تقييم، fork، rebrand، credits، عربي، نشر).

**قواعد الجودة:**
- لا commit بدون اختبارات تمرّ، أو ذكر صريح للاختبار الفاشل وسببه.
- تعديلاتنا على كود upstream تكون صغيرة ومعزولة، ويُفضّل أن تكون في ملفات جديدة. هذا يقلل تعارضات الدمج.
- كل تعديل يُسجَّل في `CHANGES.md`.
- screenshot قبل أي تسليم واجهة.

- 2026-10-10 end of day: sf7aat web build/deploy started (gh-pages, sf7aat.shibshib.art). Next session: verify the site, switch the sf7aat card from "قريباً" to "جرّبه الآن", delete sf7aat/target.

- 2026-10-11: الخط الإنجليزي للهوية صار **Bricolage Grotesque** (OFL، مجاني تجارياً) بدل Nobulina (رخصته شخصية بس، والتجارية 249$). الـ wordmark بكل اللوغوهات ExtraBold (outlines)، والموقع كله بالإنجليزي فيه. العربي ضل IBM Plex Sans Arabic. السكربت: `wm.py` (بالـ scratchpad، مش محفوظ). نسخ الويب والديسكتوب للبرامج بتاخذ اللوغو الجديد مع البناء الجاي.
