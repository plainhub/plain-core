import{Ft as e,U as t,b as n,d as r,et as i,f as a,lt as o,m as s,ot as c,p as l,t as u,v as d,y as f,z as p,zt as m}from"./chunk-_plugin-vue_export-helper-BqdUuCM-.js";import{f as h,t as g}from"./chunk-runtime-dom.esm-bundler-9r2wR3cO.js";import{t as _}from"./chunk-eventbus-Ca-q0d_H.js";import{t as v}from"./chunk-VCheckbox-3cG2pmhZ.js";import{t as y}from"./chunk-VCircularProgress-DzB8DDfU.js";import{t as b}from"./chunk-VIconButton-BS74UPJ1.js";import{t as x}from"./chunk-VFilledButton-C9ufm-Hc.js";import{t as S}from"./chunk-VOutlinedButton-o6HlrABr.js";import{n as C,t as w}from"./chunk-search-rounded-BjpYh1FE.js";import{t as ee}from"./chunk-VDropdown-DmxsS5IJ.js";import{n as te,r as T,t as E}from"./chunk-VSelect-C9qa1V62.js";import{t as D}from"./chunk-VModal-CRwV7k7z.js";import{n as O,t as k}from"./chunk-more-vert-DCyJxIG9.js";import{n as A,t as j}from"./chunk-music-CtGJ0GRL.js";import{t as M}from"./chunk-image-D68k1goC.js";import{t as N}from"./chunk-VPagination-BMDiJ7_z.js";import{n as P,t as F}from"./chunk-DangerAction-CbNBVpbj.js";import{t as I}from"./chunk-MarkdownEditor-C7_VK8kt.js";var L={viewBox:`0 0 24 24`,width:`1.2em`,height:`1.2em`};function R(e,t){return p(),s(`svg`,L,[...t[0]||=[r(`path`,{fill:`currentColor`,d:`M10.825 22q-.675 0-1.162-.45t-.588-1.1L8.85 18.8q-.325-.125-.612-.3t-.563-.375l-1.55.65q-.625.275-1.25.05t-.975-.8l-1.175-2.05q-.35-.575-.2-1.225t.675-1.075l1.325-1Q4.5 12.5 4.5 12.337v-.675q0-.162.025-.337l-1.325-1Q2.675 9.9 2.525 9.25t.2-1.225L3.9 5.975q.35-.575.975-.8t1.25.05l1.55.65q.275-.2.575-.375t.6-.3l.225-1.65q.1-.65.588-1.1T10.825 2h2.35q.675 0 1.163.45t.587 1.1l.225 1.65q.325.125.613.3t.562.375l1.55-.65q.625-.275 1.25-.05t.975.8l1.175 2.05q.35.575.2 1.225t-.675 1.075l-1.325 1q.025.175.025.338v.674q0 .163-.05.338l1.325 1q.525.425.675 1.075t-.2 1.225l-1.2 2.05q-.35.575-.975.8t-1.25-.05l-1.5-.65q-.275.2-.575.375t-.6.3l-.225 1.65q-.1.65-.587 1.1t-1.163.45zm1.225-6.5q1.45 0 2.475-1.025T15.55 12t-1.025-2.475T12.05 8.5q-1.475 0-2.488 1.025T8.55 12t1.013 2.475T12.05 15.5`},null,-1)]])}var z=c({name:`material-symbols-settings-rounded`,render:R}),B={class:`ux-item`},V={class:`ux-demo`},H={class:`ux-code`},U=u(n({__name:`UxCodeBlock`,props:{code:{}},setup(e){return(n,i)=>(p(),s(`div`,B,[r(`div`,V,[t(n.$slots,`default`,{},void 0,!0)]),r(`pre`,H,[r(`code`,null,m(e.code),1)])]))}}),[[`__scopeId`,`data-v-209d99ba`]]),W={class:`ux-section`},G={class:`button-group`},K=n({__name:`UxButtons`,setup(t){let n=o(`a`);return(t,a)=>{let o=x,c=S,l=z,u=b,m=T,h=C;return p(),s(`section`,W,[a[10]||=r(`h2`,null,`Buttons`,-1),f(U,{code:`<v-filled-button>Primary</v-filled-button>`},{default:i(()=>[f(o,null,{default:i(()=>[...a[3]||=[d(`Primary`,-1)]]),_:1}),f(o,{disabled:``},{default:i(()=>[...a[4]||=[d(`Disabled`,-1)]]),_:1}),f(o,{loading:!0},{default:i(()=>[...a[5]||=[d(`Loading`,-1)]]),_:1})]),_:1}),f(U,{code:`<v-outlined-button>Secondary</v-outlined-button>`},{default:i(()=>[f(c,null,{default:i(()=>[...a[6]||=[d(`Secondary`,-1)]]),_:1}),f(c,{disabled:``},{default:i(()=>[...a[7]||=[d(`Disabled`,-1)]]),_:1}),f(c,{loading:!0},{default:i(()=>[...a[8]||=[d(`Loading`,-1)]]),_:1})]),_:1}),f(U,{code:`<v-outlined-button class="danger">Danger</v-outlined-button>`},{default:i(()=>[f(c,{class:`danger`},{default:i(()=>[...a[9]||=[d(`Danger`,-1)]]),_:1})]),_:1}),f(U,{code:`<v-icon-button>
  <i-material-symbols:settings-rounded />
</v-icon-button>`},{default:i(()=>[f(u,null,{default:i(()=>[f(l)]),_:1}),f(u,null,{default:i(()=>[f(m)]),_:1}),f(u,null,{default:i(()=>[f(h)]),_:1}),f(u,{loading:!0})]),_:1}),a[11]||=r(`h3`,null,`.button-group`,-1),f(U,{code:`<div class="button-group">
  <button :class="{ selected: active === 'a' }">A</button>
  <button :class="{ selected: active === 'b' }">B</button>
</div>`},{default:i(()=>[r(`div`,G,[r(`button`,{class:e({selected:n.value===`a`}),onClick:a[0]||=e=>n.value=`a`},`Option A`,2),r(`button`,{class:e({selected:n.value===`b`}),onClick:a[1]||=e=>n.value=`b`},`Option B`,2),r(`button`,{class:e({selected:n.value===`c`}),onClick:a[2]||=e=>n.value=`c`},`Option C`,2)])]),_:1})])}}}),q={class:`ux-section`},J={class:`form-row`,style:{width:`100%`}},Y=`<v-select v-model="val" label="Select" :options="[
  { label: 'Option 1', value: '1' },
  { label: 'Option 2', value: '2' },
]" />`,X=n({__name:`UxForms`,setup(e){let t=o(``),n=o(``),a=o(``),c=o(``),l=o(``),u=o(!1),h=o(``),g=o(``),_=[{label:`Option 1`,value:`1`},{label:`Option 2`,value:`2`},{label:`Option 3`,value:`3`}];return(e,o)=>{let y=te,b=E,x=v;return p(),s(`section`,q,[o[13]||=r(`h2`,null,`Form Elements`,-1),f(U,{code:`<v-text-field v-model="text" label="Label" />`},{default:i(()=>[f(y,{modelValue:t.value,"onUpdate:modelValue":o[0]||=e=>t.value=e,label:`Label`},null,8,[`modelValue`])]),_:1}),f(U,{code:`<v-text-field v-model="text" placeholder="Placeholder" />`},{default:i(()=>[f(y,{modelValue:n.value,"onUpdate:modelValue":o[1]||=e=>n.value=e,placeholder:`Enter something...`},null,8,[`modelValue`])]),_:1}),f(U,{code:`<v-text-field v-model="text" error error-text="Required field" />`},{default:i(()=>[f(y,{modelValue:a.value,"onUpdate:modelValue":o[2]||=e=>a.value=e,label:`With Error`,error:``,"error-text":`This field is required`},null,8,[`modelValue`])]),_:1}),f(U,{code:`<v-text-field v-model="text" type="textarea" :rows="3" label="Textarea" />`},{default:i(()=>[f(y,{modelValue:c.value,"onUpdate:modelValue":o[3]||=e=>c.value=e,type:`textarea`,rows:3,label:`Textarea`},null,8,[`modelValue`])]),_:1}),f(U,{code:Y},{default:i(()=>[f(b,{modelValue:l.value,"onUpdate:modelValue":o[4]||=e=>l.value=e,label:`Select`,options:_},null,8,[`modelValue`])]),_:1}),f(U,{code:`<v-checkbox v-model="checked" />`},{default:i(()=>[f(x,{modelValue:u.value,"onUpdate:modelValue":o[5]||=e=>u.value=e},null,8,[`modelValue`]),o[8]||=d(),r(`span`,null,`Checkbox (`+m(u.value?`checked`:`unchecked`)+`)`,1)]),_:1}),f(U,{code:`<v-checkbox indeterminate />`},{default:i(()=>[f(x,{indeterminate:``}),o[9]||=d(),o[10]||=r(`span`,null,`Indeterminate`,-1)]),_:1}),o[14]||=r(`h3`,null,`CSS: .form-row / .form-label`,-1),f(U,{code:`<div class="form-row">
  <div><label class="form-label">Name</label>
    <v-text-field ... /></div>
  <div><label class="form-label">Email</label>
    <v-text-field ... /></div>
</div>`},{default:i(()=>[r(`div`,J,[r(`div`,null,[o[11]||=r(`label`,{class:`form-label`},`Name`,-1),f(y,{modelValue:h.value,"onUpdate:modelValue":o[6]||=e=>h.value=e,placeholder:`Name`},null,8,[`modelValue`])]),r(`div`,null,[o[12]||=r(`label`,{class:`form-label`},`Email`,-1),f(y,{modelValue:g.value,"onUpdate:modelValue":o[7]||=e=>g.value=e,placeholder:`Email`},null,8,[`modelValue`])])])]),_:1})])}}}),Z={class:`ux-section`},Q={style:{width:`100%`}},ne=n({__name:`UxFeedback`,setup(e){return(e,t)=>{let n=y,a=P,o=F;return p(),s(`section`,Z,[t[3]||=r(`h2`,null,`Feedback & Progress`,-1),f(U,{code:`<v-circular-progress indeterminate />
<v-circular-progress indeterminate class="sm" />`},{default:i(()=>[f(n,{indeterminate:``}),f(n,{indeterminate:``,class:`sm`}),t[0]||=r(`span`,{style:{"font-size":`0.8rem`,color:`var(--md-sys-color-on-surface-variant)`}},`default / .sm`,-1)]),_:1}),f(U,{code:`<v-circular-progress indeterminate class="primary" />
<v-circular-progress indeterminate class="error" />`},{default:i(()=>[f(n,{indeterminate:``,class:`primary`}),f(n,{indeterminate:``,class:`secondary`}),f(n,{indeterminate:``,class:`error`})]),_:1}),f(U,{code:`<progress-card label-html="Uploading <b>file.zip</b>" :value="65" />`},{default:i(()=>[f(a,{"label-html":`Uploading <b>file.zip</b>`,value:65})]),_:1}),t[4]||=r(`h3`,null,`CSS: .progress-track / .progress-fill`,-1),f(U,{code:`<div class="progress-track">
  <div class="progress-fill" :style="{ width: '60%' }" />
</div>`},{default:i(()=>[...t[1]||=[r(`div`,{style:{width:`100%`}},[r(`div`,{class:`progress-track`},[r(`div`,{class:`progress-fill`,style:{width:`60%`}})])],-1)]]),_:1}),t[5]||=r(`h3`,null,`<danger-action>`,-1),f(U,{code:`<danger-action label="Delete account" confirm-text="Are you sure?" @confirm="..." />`},{default:i(()=>[r(`div`,Q,[f(o,{label:`Delete account`,"confirm-text":`Are you sure?`,onConfirm:()=>{}})])]),_:1}),t[6]||=r(`h3`,null,`CSS: .alert-danger / .alert-info / .alert-warning`,-1),f(U,{code:`<div class="alert-info show">Info alert message</div>
<div class="alert-danger show">Danger alert</div>
<div class="alert-warning show">Warning alert</div>`},{default:i(()=>[...t[2]||=[r(`div`,{style:{width:`100%`,display:`flex`,"flex-direction":`column`,gap:`8px`}},[r(`div`,{class:`alert-info show`},`Info: Something informational happened.`),r(`div`,{class:`alert-danger show`},[r(`svg`,{xmlns:`http://www.w3.org/2000/svg`,width:`20`,height:`20`,viewBox:`0 0 24 24`,fill:`none`,stroke:`currentColor`,"stroke-width":`2`},[r(`circle`,{cx:`12`,cy:`12`,r:`10`}),r(`line`,{x1:`15`,y1:`9`,x2:`9`,y2:`15`}),r(`line`,{x1:`9`,y1:`9`,x2:`15`,y2:`15`})]),r(`div`,{class:`alert-body`},`Danger: Something went wrong.`)])],-1)]]),_:1})])}}}),re={class:`ux-section`},ie=`<v-modal v-if="show" @close="show = false">
  <template #headline>Title</template>
  <template #content>Content here</template>
  <template #actions>
    <v-outlined-button @click="show = false">Cancel</v-outlined-button>
    <v-filled-button @click="show = false">OK</v-filled-button>
  </template>
</v-modal>`,ae=`<v-dropdown v-model="open">
  <template #trigger>
    <v-outlined-button>Menu</v-outlined-button>
  </template>
  <div class="dropdown-item">Action One</div>
  <div class="dropdown-item selected">Selected</div>
  <div class="dropdown-item danger">Danger</div>
</v-dropdown>`,oe=n({__name:`UxOverlays`,setup(e){let t=o(!1),n=o(!1),c=o(!1);return(e,o)=>{let u=x,m=S,h=D,g=ee,_=k,v=b;return p(),s(`section`,re,[o[19]||=r(`h2`,null,`Overlays`,-1),f(U,{code:ie},{default:i(()=>[f(u,{onClick:o[0]||=e=>t.value=!0},{default:i(()=>[...o[13]||=[d(`Open Modal`,-1)]]),_:1}),t.value?(p(),a(h,{key:0,onClose:o[3]||=e=>t.value=!1},{headline:i(()=>[...o[14]||=[d(`Modal Title`,-1)]]),content:i(()=>[...o[15]||=[r(`p`,null,`This is the modal content area. You can put any content here.`,-1)]]),actions:i(()=>[f(m,{onClick:o[1]||=e=>t.value=!1},{default:i(()=>[...o[16]||=[d(`Cancel`,-1)]]),_:1}),f(u,{onClick:o[2]||=e=>t.value=!1},{default:i(()=>[...o[17]||=[d(`Confirm`,-1)]]),_:1})]),_:1})):l(``,!0)]),_:1}),o[20]||=r(`h3`,null,`<v-dropdown> + .dropdown-item`,-1),f(U,{code:ae},{default:i(()=>[f(g,{modelValue:n.value,"onUpdate:modelValue":o[9]||=e=>n.value=e},{trigger:i(()=>[f(m,{onClick:o[4]||=e=>n.value=!n.value},{default:i(()=>[...o[18]||=[d(`Open Dropdown`,-1)]]),_:1})]),default:i(()=>[r(`div`,{class:`dropdown-item`,onClick:o[5]||=e=>n.value=!1},`Action One`),r(`div`,{class:`dropdown-item`,onClick:o[6]||=e=>n.value=!1},`Action Two`),r(`div`,{class:`dropdown-item selected`,onClick:o[7]||=e=>n.value=!1},`Selected Item`),r(`div`,{class:`dropdown-item danger`,onClick:o[8]||=e=>n.value=!1},`Danger Item`)]),_:1},8,[`modelValue`])]),_:1}),f(U,{code:`<!-- Dropdown with icon button trigger -->
<v-dropdown v-model="open">
  <template #trigger>
    <v-icon-button>
      <i-material-symbols:more-vert />
    </v-icon-button>
  </template>
  <div class="dropdown-item">Edit</div>
  <div class="dropdown-item danger">Delete</div>
</v-dropdown>`},{default:i(()=>[f(g,{modelValue:c.value,"onUpdate:modelValue":o[12]||=e=>c.value=e},{trigger:i(()=>[f(v,null,{default:i(()=>[f(_)]),_:1})]),default:i(()=>[r(`div`,{class:`dropdown-item`,onClick:o[10]||=e=>c.value=!1},`Edit`),r(`div`,{class:`dropdown-item danger`,onClick:o[11]||=e=>c.value=!1},`Delete`)]),_:1},8,[`modelValue`])]),_:1})])}}}),se=[`tabindex`],ce={key:0,class:`v-input-chip__icon`},le={class:`v-input-chip__label`},ue=[`disabled`,`aria-label`],de=u(n({__name:`VInputChip`,props:{label:{default:``},selected:{type:Boolean,default:!1},disabled:{type:Boolean,default:!1},removeOnly:{type:Boolean,default:!1},ariaLabelRemove:{default:``}},emits:[`remove`,`click`],setup(n,{emit:i}){let a=n,o=i;function c(){!a.disabled&&!a.removeOnly&&o(`click`)}function u(){a.disabled||o(`remove`)}function d(e){(e.key===`Enter`||e.key===` `)&&(e.preventDefault(),c())}return(i,a)=>(p(),s(`div`,{class:e([`v-input-chip`,{"v-input-chip--selected":n.selected,"v-input-chip--disabled":n.disabled,"v-input-chip--remove-only":n.removeOnly}]),tabindex:n.disabled?-1:0,onClick:c,onKeydown:d},[i.$slots.default?(p(),s(`span`,ce,[t(i.$slots,`default`,{},void 0,!0)])):l(``,!0),r(`span`,le,m(n.label),1),r(`button`,{class:`v-input-chip__remove`,type:`button`,disabled:n.disabled,"aria-label":n.ariaLabelRemove||`Remove`,onClick:h(u,[`stop`])},[...a[0]||=[r(`svg`,{viewBox:`0 0 24 24`,width:`18`,height:`18`},[r(`path`,{d:`M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z`})],-1)]],8,ue)],42,se))}}),[[`__scopeId`,`data-v-aa119b53`]]),fe={},pe={class:`v-chip-set`};function me(e,n){return p(),s(`div`,pe,[t(e.$slots,`default`,{},void 0,!0)])}var he=u(fe,[[`render`,me],[`__scopeId`,`data-v-113a3b29`]]),ge={class:`chip-label`},_e=u(n({__name:`VFilterChip`,props:{label:{},selected:{type:Boolean,default:!1}},emits:[`click`],setup(t){return(n,i)=>(p(),s(`button`,{class:e([`v-filter-chip`,{selected:t.selected}]),onClick:i[0]||=e=>n.$emit(`click`,e)},[r(`span`,ge,m(t.label),1)],2))}}),[[`__scopeId`,`data-v-f63654bf`]]),ve={class:`ux-section`},ye={class:`top-app-bar`,style:{width:`100%`}},be={class:`actions`},xe=n({__name:`UxDataDisplay`,setup(e){let t=o(`all`);return(e,n)=>{let a=_e,o=he,c=de,l=O,u=b,d=w,m=k;return p(),s(`section`,ve,[n[6]||=r(`h2`,null,`Chips & Data Display`,-1),n[7]||=r(`h3`,null,`<v-filter-chip> + <v-chip-set>`,-1),f(U,{code:`<v-chip-set>
  <v-filter-chip label="All" :selected="filter === 'all'" @click="filter = 'all'" />
  <v-filter-chip label="Active" :selected="filter === 'active'" @click="filter = 'active'" />
</v-chip-set>`},{default:i(()=>[f(o,null,{default:i(()=>[f(a,{label:`All`,selected:t.value===`all`,onClick:n[0]||=e=>t.value=`all`},null,8,[`selected`]),f(a,{label:`Active`,selected:t.value===`active`,onClick:n[1]||=e=>t.value=`active`},null,8,[`selected`]),f(a,{label:`Archived`,selected:t.value===`archived`,onClick:n[2]||=e=>t.value=`archived`},null,8,[`selected`])]),_:1})]),_:1}),n[8]||=r(`h3`,null,`<v-input-chip>`,-1),f(U,{code:`<v-input-chip label="Tag" @remove="..." />`},{default:i(()=>[f(c,{label:`Photo`,onRemove:()=>{}}),f(c,{label:`Video`,selected:``,onRemove:()=>{}}),f(c,{label:`Disabled`,disabled:``})]),_:1}),n[9]||=r(`h3`,null,`CSS: .card / .card.outlined`,-1),f(U,{code:`<div class="card">Default card</div>
<div class="card outlined">Outlined card</div>
<div class="card selected">Selected card</div>`},{default:i(()=>[...n[3]||=[r(`div`,{class:`card`,style:{flex:`1`}},`Default card content`,-1),r(`div`,{class:`card outlined`,style:{flex:`1`}},`Outlined card content`,-1),r(`div`,{class:`card selected`,style:{flex:`1`}},`Selected card content`,-1)]]),_:1}),n[10]||=r(`h3`,null,`CSS: .selectable-card`,-1),f(U,{code:`<div class="selectable-card">Normal</div>
<div class="selectable-card selected">Selected</div>
<div class="selectable-card selecting">Selecting</div>`},{default:i(()=>[...n[4]||=[r(`div`,{class:`selectable-card`,style:{padding:`12px`,flex:`1`}},`Normal`,-1),r(`div`,{class:`selectable-card selected`,style:{padding:`12px`,flex:`1`}},`Selected`,-1),r(`div`,{class:`selectable-card selecting`,style:{padding:`12px`,flex:`1`}},`Selecting`,-1)]]),_:1}),n[11]||=r(`h3`,null,`CSS: .top-app-bar`,-1),f(U,{code:`<div class="top-app-bar">
  <v-icon-button>...</v-icon-button>
  <div class="title">Page Title</div>
  <div class="actions">...</div>
</div>`},{default:i(()=>[r(`div`,ye,[f(u,null,{default:i(()=>[f(l)]),_:1}),n[5]||=r(`div`,{class:`title`},`Page Title`,-1),r(`div`,be,[f(u,null,{default:i(()=>[f(d)]),_:1}),f(u,null,{default:i(()=>[f(m)]),_:1})])])]),_:1})])}}}),Se={class:`ux-section`},Ce={style:{display:`flex`,"flex-direction":`column`,gap:`8px`,width:`100%`}},we={style:{display:`flex`,gap:`16px`,"align-items":`flex-start`}},Te={key:0,class:`card`,style:{padding:`8px`}},Ee={key:0,class:`card`,style:{padding:`8px`,overflow:`hidden`}},De={key:0,class:`card`,style:{padding:`8px`,overflow:`hidden`}},$={class:`nav`,style:{width:`200px`}},Oe={class:`active`},ke={class:`icon`},Ae={class:`icon`},je={class:`icon`},Me=n({__name:`UxCssUtilities`,setup(e){let t=o(!0),n=o(3);return(e,a)=>{let o=S,c=A,u=M,h=j,_=N;return p(),s(`section`,Se,[a[12]||=r(`h2`,null,`CSS Utilities`,-1),a[13]||=r(`h3`,null,`.skeleton-text / .skeleton-image`,-1),f(U,{code:`<div class="skeleton-text" style="width: 60%" />
<div class="skeleton-text lg" style="width: 40%" />
<div class="skeleton-image lg" />
<div class="skeleton-checkbox" />`},{default:i(()=>[...a[1]||=[r(`div`,{style:{display:`flex`,gap:`12px`,"align-items":`center`,width:`100%`}},[r(`div`,{class:`skeleton-image lg`}),r(`div`,{style:{flex:`1`,display:`flex`,"flex-direction":`column`,gap:`8px`}},[r(`div`,{class:`skeleton-text`,style:{width:`70%`}}),r(`div`,{class:`skeleton-text lg`,style:{width:`45%`}})]),r(`div`,{class:`skeleton-checkbox`})],-1)]]),_:1}),a[14]||=r(`h3`,null,`.nowrap`,-1),f(U,{code:`<span class="nowrap">Text that won't wrap</span>`},{default:i(()=>[...a[2]||=[r(`span`,{class:`nowrap`},`This text will not wrap to a new line, no matter what.`,-1)]]),_:1}),a[15]||=r(`h3`,null,`Vue Transitions: fade / width / height`,-1),f(U,{code:`<transition name="fade">
  <div v-if="show">Fades in/out</div>
</transition>

<transition name="width">
  <div v-if="show">Width transition</div>
</transition>

<transition name="height">
  <div v-if="show">Height transition</div>
</transition>`},{default:i(()=>[r(`div`,Ce,[f(o,{onClick:a[0]||=e=>t.value=!t.value},{default:i(()=>[d(` Toggle (`+m(t.value?`visible`:`hidden`)+`) `,1)]),_:1}),r(`div`,we,[r(`div`,null,[a[3]||=r(`div`,{style:{"font-size":`0.75rem`,"margin-bottom":`4px`,color:`var(--md-sys-color-on-surface-variant)`}},`fade:`,-1),f(g,{name:`fade`},{default:i(()=>[t.value?(p(),s(`div`,Te,`Fade`)):l(``,!0)]),_:1})]),r(`div`,null,[a[4]||=r(`div`,{style:{"font-size":`0.75rem`,"margin-bottom":`4px`,color:`var(--md-sys-color-on-surface-variant)`}},`width:`,-1),f(g,{name:`width`},{default:i(()=>[t.value?(p(),s(`div`,Ee,`Width`)):l(``,!0)]),_:1})]),r(`div`,null,[a[5]||=r(`div`,{style:{"font-size":`0.75rem`,"margin-bottom":`4px`,color:`var(--md-sys-color-on-surface-variant)`}},`height:`,-1),f(g,{name:`height`},{default:i(()=>[t.value?(p(),s(`div`,De,`Height`)):l(``,!0)]),_:1})])])])]),_:1}),a[16]||=r(`h3`,null,`.surface-card`,-1),f(U,{code:`<div class="surface-card">Surface card with background</div>`},{default:i(()=>[...a[6]||=[r(`div`,{class:`surface-card`,style:{flex:`1`}},`Surface card content with default padding`,-1)]]),_:1}),a[17]||=r(`h3`,null,`.nav (sidebar navigation)`,-1),f(U,{code:`<ul class="nav">
  <li class="active">
    <div class="icon">📁</div>
    <div class="title">Files</div>
    <div class="count">12</div>
  </li>
</ul>`},{default:i(()=>[r(`ul`,$,[r(`li`,Oe,[r(`div`,ke,[f(c)]),a[7]||=r(`div`,{class:`title`},`Files`,-1),a[8]||=r(`div`,{class:`count`},`12`,-1)]),r(`li`,null,[r(`div`,Ae,[f(u)]),a[9]||=r(`div`,{class:`title`},`Images`,-1),a[10]||=r(`div`,{class:`count`},`48`,-1)]),r(`li`,null,[r(`div`,je,[f(h)]),a[11]||=r(`div`,{class:`title`},`Audio`,-1)])])]),_:1}),a[18]||=r(`h3`,null,`.pagination`,-1),f(U,{code:`<v-pagination :total="100" :limit="10" :page="3" :go="goToPage" />`},{default:i(()=>[f(_,{total:100,limit:10,page:n.value,go:e=>n.value=e},null,8,[`page`,`go`])]),_:1})])}}}),Ne={class:`ux-md-editor`},Pe={class:`toolbar`},Fe={class:`editor-frame`},Ie=u(n({__name:`UxMarkdownEditor`,setup(e){let t=o(`# 产品设计评审

本周与团队同步了 **PlainDesk 2.0** 的整体方向，重点是 *编辑器* 的交互模型。参考资料见 [发布计划](https://example.com)，接口约定在 \`useNoteEdit.ts\`。

## 待办事项

- [x] 整理竞品交互对比
- [ ] 绘制 Live Preview 高保真稿
- [ ] ~~手写富文本解析器~~ 改用 syntaxTree

\`\`\`ts
const view = new EditorView({ state, parent })
\`\`\`

缩进代码：

    indented code line
    second line

![架构图](data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='360'%20height='120'%3E%3Crect%20width='360'%20height='120'%20fill='%234f5f9e'/%3E%3Ctext%20x='180'%20y='68'%20font-size='22'%20fill='white'%20text-anchor='middle'%20font-family='sans-serif'%3EPlainDesk%20Architecture%3C/text%3E%3C/svg%3E)

| 指标 | 冷启动 | 滚动 |
| :--- | ---: | :---: |
| 改造前 | 240ms | 12% 掉帧 |
| **工具** | \`vite\` | ~~installed~~ |
| 改造后 | 90ms | 0 掉帧 |

行内公式 $E = mc^2$ 与积分：

$$
\\int_0^\\infty e^{-x} \\, dx = 1
$$

---

---

## Table of Contents

*   [Method 1: PlainApp — no app on the iPhone](#method-1-plainapp-no-app-on-the-iphone)
*   [Method 2: LocalSend — an app on both sides](#method-2-localsend-an-app-on-both-sides)

![Diagram 1](data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='360'%20height='80'%3E%3Crect%20width='360'%20height='80'%20fill='%2300897b'/%3E%3Ctext%20x='180'%20y='48'%20font-size='20'%20fill='white'%20text-anchor='middle'%20font-family='sans-serif'%3EDiagram%201%3C/text%3E%3C/svg%3E)

## Method 1: PlainApp — no app on the iPhone

方法一的内容段落。

> 编辑器是笔记应用的主战场。
`),n=o(document.documentElement.classList.contains(`dark`));function i(){n.value=!n.value,document.documentElement.classList.toggle(`dark`,n.value),_.emit(`color_mode_changed`)}return(e,a)=>(p(),s(`section`,Ne,[a[1]||=r(`h2`,null,`MarkdownEditor (Live Preview)`,-1),a[2]||=r(`p`,{class:`hint`},[d(` 光标所在元素显示原始 Markdown 标记，移开后原地渲染；空行输入 `),r(`code`,null,`/`),d(` 呼出块插入菜单。 `)],-1),r(`div`,Pe,[r(`button`,{type:`button`,onClick:i},m(n.value?`☀️ Light`:`🌙 Dark`),1)]),r(`div`,Fe,[f(I,{modelValue:t.value,"onUpdate:modelValue":a[0]||=e=>t.value=e,placeholder:`Write markdown... (input / for blocks)`},null,8,[`modelValue`])])]))}}),[[`__scopeId`,`data-v-8f4d67fe`]]),Le={class:`ux-page`},Re={class:`ux-main`},ze=u(n({__name:`UxView`,setup(e){return(e,t)=>(p(),s(`div`,Le,[t[0]||=r(`header`,{class:`ux-header`},[r(`h1`,null,`UI Component Library`),r(`p`,null,`Base components and reusable CSS patterns. All components are globally registered with V-prefix.`)],-1),r(`main`,Re,[f(K),f(X),f(ne),f(oe),f(xe),f(Me),f(Ie)])]))}}),[[`__scopeId`,`data-v-b578e6b5`]]);export{ze as default};