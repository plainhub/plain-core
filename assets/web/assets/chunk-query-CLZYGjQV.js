import{r as e,t}from"./chunk-native-invoke-DYRnWgQ8.js";import{a as n,r}from"./index-DNjP8kkU.js";import{a as i,o as a}from"./chunk-client-id-JJHYexBT.js";import{W as o,ct as s,d as c,lt as l,n as u,z as d}from"./chunk-http-CRorJ5ja.js";import{_ as f}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{A as p,F as m,L as h,j as g,tt as _,x as v}from"./chunk-_plugin-vue_export-helper-gwY6GXv8.js";import{C as ee,I as y,L as b,c as te,i as x}from"./chunk-gql-client-BLVkp8Nn.js";import{t as S}from"./chunk-websocket-crypto-BrqQXtPp.js";var C=`
  fragment MountFragment on Mount {
    id
    name
    path
    mountPoint
    fsType
    totalBytes
    usedBytes
    freeBytes
    remote
    alias
    driveType
    diskId
    label
    diskModel
  }
`,w=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,T=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,E=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,D=`
  fragment AppFragment on App {
    clientId
    urlToken
    httpPort
    httpsPort
    appDir
    deviceName
    deviceType
    capabilities
    access
    downloadsDir
    developerMode
    debug
  }
`,O=`
  fragment ChatItemFragment on ChatItem {
    id
    fromId
    toId
    channelId
    createdAt
    content { ... on ChatMessageContent { __typename text format linkPreviews { url title description image imageWidth imageHeight siteName domain createdAt } items { type attachment { id uri size durationMs width height summary fileName } } } ... on ChatShareContent { __typename shareId urlToken peerInfo { id ip port } name itemCount totalSize expiresAt } }
    status
    deliveryResults { peerId peerName error }
  }
`,ne=`
  fragment SmsFragment on Sms {
    id
    body
    address
    serviceCenter
    sentAt
    type
    threadId
    subscriptionId
    isMms
    attachments {
      path
      contentType
      name
    }
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,k=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,A=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,j=`
  fragment ContactFragment on Contact {
    id
    suffix
    prefix
    firstName
    middleName
    lastName
    updatedAt
    notes
    source
    thumbnailId
    starred
    phoneNumbers {
      value
      type
      label
      normalizedNumber
    }
    addresses {
      value
      type
      label
    }
    emails {
      value
      type
      label
    }
    websites {
      value
      type
      label
    }
    events {
      value
      type
      label
    }
    ims {
      value
      protocol
      customProtocol
    }
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,M=`
  fragment CallFragment on Call {
    id
    name
    number
    durationSec
    accountId
    startedAt
    photo
    type
    geo {
      country
      numberType
      carrier
      description
    }
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,N=`
  fragment FileFragment on File {
    isSystem
    path
    isDir
    createdAt
    updatedAt
    size
    childCount
    mediaId
  }
`,re=`
  fragment ImageFragment on Image {
    id
    title
    path
    size
    bucketId
    takenAt
    createdAt
    updatedAt
    thumbHash
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,ie=`
  fragment VideoFragment on Video {
    id
    title
    path
    durationMs
    size
    bucketId
    createdAt
    updatedAt
    takenAt
    thumbHash
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,ae=`
  fragment AudioFragment on Audio {
    id
    title
    artist
    path
    durationMs
    size
    bucketId
    albumFileId
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,P=`
  fragment NoteFragment on Note {
    id
    title
    content
    deletedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,oe=`
  fragment DocFragment on Doc {
    id
    title
    path
    extension
    size
    bucketId
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,F=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,I=`
  fragment FeedEntryFragment on FeedEntry {
    id
    title
    url
    image
    author
    description
    content
    feedId
    publishedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${T}
`,L=`
  fragment PackageFragment on Package {
    id
    name
    type
    version
    path
    size
    certs {
      issuer
      subject
      serialNumber
      validFrom
      validTo
    }
    installedAt
    updatedAt
  }
`,R=`
  fragment NotificationFragment on Notification {
    id
    onlyOnce
    isClearable
    appId
    appName
    postedAt
    silent
    title
    body
    actions
    replyActions
  }
`,z=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,B=`
  fragment DeviceInfoFragment on DeviceInfo {
    name
    platform
    manufacturer
    model
    osName
    osVersion
    kernelVersion
    appVersion
    appBuildNumber
    language
    cpuArch
    cpuModel
    totalMemory
    totalStorage
    display {
      width
      height
      density
    }
    android {
      sdkVersion
      versionCodeName
      securityPatch
      bootloader
      fingerprint
      hardware
      radioVersion
      board
      buildBrand
      buildNumber
      device
      javaVmVersion
      glEsVersion
      buildTime
    }
  }
`,V=`
  fragment DeviceStatusFragment on DeviceStatus {
    uptimeSec
    batteryLevel
    charging
    temperatures {
      label
      celsius
    }
    cpuUsage
    memoryAvailable
    storageAvailable
  }
`,H=`
  fragment BookmarkFragment on Bookmark {
    id
    url
    title
    faviconPath
    groupId
    pinned
    clickCount
    lastClickedAt
    sortOrder
    createdAt
    updatedAt
  }
`,U=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,W=`
  fragment ChatChannelFragment on ChatChannel {
    actions { isOwner canRename canManageMembers canLeave canDelete }
    id
    name
    ownerId
    members {
      ...ChatChannelMemberFragment
    }
    version
    status
    createdAt
    updatedAt
  }

  fragment ChatChannelMemberFragment on ChatChannelMember {
    peerId
    status
  }

`,G=function(e){return e.InvalidSubscription=`INVALID_SUBSCRIPTION`,e.SubscriptionResultTooLarge=`SUBSCRIPTION_RESULT_TOO_LARGE`,e.ResyncRequired=`RESYNC_REQUIRED`,e.SlowClient=`SLOW_CLIENT`,e}({}),K=new Set(Object.values(G));function q(e){return typeof e==`string`&&K.has(e)?e:void 0}function J(e,t){return se(e,t)}function se(e,t){let n=d(`/graphql`,t),r=new URL(n,c()),s=i(),l=o();if(!l)throw Error(`Subscription session missing`);r.searchParams.set(`cid`,s);let f=u(r.toString(),a());f.binaryType=`arraybuffer`;let p=!0,m=!1,h,g,_,v=setTimeout(()=>b(Error(`Subscription handshake timeout`)),1e4),y={send(e){try{if(!p||!m||!h)throw Error(`Subscription connection unavailable`);n!==`/graphql`&&ce(e)&&(e={...e,payload:{...e.payload,query:te(e.payload.query)}}),f.send(h.encode(new TextEncoder().encode(JSON.stringify(e))))}catch(e){b(e)}},close(){p=!1,h=void 0,clearTimeout(v),clearTimeout(_),clearInterval(g),f.close()}};function b(t){p&&(y.close(),e.dropped(t))}return f.onopen=()=>{if(p)try{h=new S(s,ee(l)),f.send(h.registration)}catch(e){b(e)}},f.onmessage=t=>{if(p)try{if(!h||!(t.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(m){let n=JSON.parse(new TextDecoder().decode(h.decode(new Uint8Array(t.data))));typeof n==`object`&&n&&`type`in n&&n.type===`pong`?clearTimeout(_):e.message(n)}else{if(h.acknowledge(new Uint8Array(t.data)),f.send(h.encode(new Uint8Array)),m=!0,clearTimeout(v),!p||(e.opened(),!p))return;g=setInterval(()=>{y.send({type:`ping`}),p&&(_=setTimeout(()=>b(Error(`Subscription heartbeat timeout`)),1e4))},2e4)}}catch(e){b(e)}},f.onerror=()=>b(Error(`Subscription connection error`)),f.onclose=()=>b(Error(`Subscription connection closed`)),y}function ce(e){if(!e||typeof e!=`object`||!(`type`in e)||e.type!==`subscribe`||!(`payload`in e))return!1;let t=e.payload;return!!t&&typeof t==`object`&&`query`in t&&typeof t.query==`string`}function le(i,a={kind:`LOCAL`}){if(a.kind===`PEER`&&!a.peerId)throw Error(n.NativePeerIdMissing);let o=!0,s,c=Promise.resolve(),l={send(e){if(!o||s===void 0)throw Error(`Native subscription unavailable`);let n=s;c=c.then(()=>o?t(`native_subscription_send`,{handle:n,message:JSON.stringify(e)}):void 0).catch(d)},close(){u(),o=!1,s!==void 0&&t(`native_subscription_close`,{handle:s}).catch(console.error),s=void 0}},u=e(l.close);function d(e){o&&(l.close(),i.dropped(e))}return(async()=>{let e=await r();if(!o)return;let n=await t(`native_subscription_open`,{scope:e,target:a});if(!o){await t(`native_subscription_close`,{handle:n});return}for(s=n,i.opened();o;){let e=await t(`native_subscription_read`,{handle:n});o&&i.message(JSON.parse(e))}})().catch(d),l}var Y=class{openConnection;retryConnection;constructor(e=J,t=()=>!l()){this.openConnection=e,this.retryConnection=t}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&(!this.attaching&&!this.pending.size&&(this.attaching=!0,this.pending=new Set(this.ids.keys())),this.settled(t.id)),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>q(e.extensions?.code)===G.ResyncRequired)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&this.retryConnection()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},X=new Y,ue={audios:`audioCount(query: $mediaQuery)`,images:`imageCount(query: $mediaQuery)`,videos:`videoCount(query: $mediaQuery)`,docs:`docCount(query: "")`,packages:`packageCount(query: "")`,notes:`noteCount(query: "")`,feedEntries:`feedEntryCount(query: "")`,messages:`smsCount(query: "")`,calls:`callCount(query: "")`,contacts:`contactCount(query: "")`};function de(e=[]){let t=e.map(e=>ue[e]).join(`
    `);return`
  query homeStats${t.includes(`$mediaQuery`)?`($mediaQuery: String!)`:``} {
    ${t}
    mounts {
      ...MountFragment
    }
  }
  ${C}
`}function Z(e){return e instanceof s?e.status===403?b.RemoteAccessDisabled:e.message:y.NetworkError}function Q(e){if(e)return typeof e==`function`?e():e}function fe(e){let t=f(!1),n=f(),r=0;async function i(i,a=!1){if(e.enabled&&!e.enabled())return;let o=r;t.value=!0;try{let t=i??Q(e.variables),s=await x(typeof e.document==`function`?e.document():e.document,t,{fresh:a});if(o!==r)return;s.errors?.length?e.handle(s.data,s.errors[0].message):(n.value=s.data,e.handle(s.data,``))}catch(t){if(o!==r||t instanceof s&&t.canceled)return;e.handle(void 0,Z(t))}finally{o===r&&(t.value=!1)}}i();let a=!0,o=v();return o&&(m(()=>{a=!1}),g(()=>{a=!0})),typeof e.variables==`function`&&_(e.variables,async()=>{await p(),a&&i()},{deep:!0}),typeof e.document==`function`&&_(e.document,async()=>{await p(),a&&i()}),e.enabled&&_(e.enabled,async(e,t)=>{await p(),a&&e&&!t&&i()}),o&&h(X.onAttached(()=>{a&&(r++,i(void 0,!0))})),{loading:t,result:n,refetch:i}}function pe(e){let t=f(!1),n=f(),r=0,i=0,a=0,o,c=0,l=!0,u;function d(){c++,a=0,o=void 0,t.value=!1}if(v()){g(()=>{l=!0}),m(()=>{l=!1,d()});let e=X.onAttached(()=>{l&&u&&p(u.variables,{...u.options,latest:!0,force:!0})});h(()=>{d(),e()})}async function p(l,f={}){f.force&&d();let p=c;u={variables:l,options:f};let m=++r,h=f.latest?++i:void 0,g=l??Q(e.variables),_=g?{...g}:void 0,v={variables:_,requestId:m,meta:f.meta};f.latest?o=m:a++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=f.force?await x(t,_,{fresh:!0}):await x(t,_);if(p!==c||f.latest&&h!==i)return;r.errors?.length?e.handle(r.data,r.errors[0].message,v):(n.value=r.data,e.handle(r.data,``,v))}catch(t){if(p!==c||t instanceof s&&t.canceled)return;(!f.latest||h===i)&&e.handle(void 0,Z(t),v)}finally{p===c&&(f.latest?o===m&&(o=void 0):a--,t.value=a>0||o!==void 0)}}return{loading:t,result:n,fetch:p}}var me=`
  query ($query: String!) {
    chatItems(query: $query, offset: 0, limit: 200, sortBy: NEWEST_FIRST) {
      ...ChatItemFragment
    }
  }
  ${O}
`;`${O}`;var he=`
  query {
    peers {
      id
      name
      ip
      status
      online
      managed
      port
      deviceType
      createdAt
      updatedAt
    }
  }
`,ge=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${O}
`,_e=`
  query appFiles($offset: Int!, $limit: Int!) {
    appFiles(query: "", offset: $offset, limit: $limit) {
      id
      size
      mimeType
      fileName
      createdAt
      updatedAt
    }
    appFileCount(query: "")
  }
`,ve=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,ye=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${W}
`,be=`
  query ($path: String!) {
    fileInfo(path: $path) {
      ... on FileInfo {
        path
        updatedAt
        size
      }
      data {
        ... on ImageFileInfo {
          width
          height
          location {
            latitude
            longitude
          }
        }
        ... on VideoFileInfo {
          durationMs
          width
          height
          location {
            latitude
            longitude
          }
        }
        ... on AudioFileInfo {
          durationMs
          location {
            latitude
            longitude
          }
        }
      }
    }
  }
`,xe=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${ne}
`,Se=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,Ce=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${k}
`,we=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${A}
`,Te=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${j}
`,Ee=`
  query {
    contactSources {
      name
      type
    }
  }
`,De=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${M}
`,Oe=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: ImageSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${re}
`,ke=`
  query {
    imageSearchStatus {
      status
      downloadProgress
      errorMessage
      modelSize
      modelDir
      isIndexing
      totalImages
      indexedImages
    }
  }
`,Ae=`
  query($volumeId: ID) {
    fileIndexScanProgress(volumeId: $volumeId) { volumeId
      indexed
      pending
      total
      state
      error
    }
  }
`,je=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: VideoSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${ie}
`,Me=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: AudioSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${ae}
`,Ne=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${N}
`,Pe=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,Fe=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${N}
`,Ie=`
  query {
    mounts {
      ...MountFragment
    }
  }
  ${C}
`,Le=`
  query {
    disks {
      id
      name
      path
      sizeBytes
      removable
      model
    }
  }
`,Re=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,ze=`
  query {
    app {
      ...AppFragment
    }
  }
  ${D}
`,Be=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Ve=`
  query audioQueue($offset: Int!, $limit: Int!) {
    items: audioQueueItems(query: "", offset: $offset, limit: $limit) {
      ...AudioItemFragment
    }
    total: audioQueueItemCount(query: "")
    playback: audioPlayback {
      mediaId
      path
    }
    prefs(keys: ["audio_play_mode"])
  }
  ${E}
`,He=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${w}
`,Ue=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      coverItems { path updatedAt thumbHash }
    }
  }
`,We=`
  query notes($offset: Int!, $limit: Int!, $query: String!) {
    notes(query: $query, offset: $offset, limit: $limit) {
      id
      title
      deletedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    noteCount(query: $query)
  }
  ${T}
`,Ge=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${P}
`,Ke=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${F}
`,qe=`
  query feedEntries($offset: Int!, $limit: Int!, $query: String!) {
    items: feedEntries(query: $query, offset: $offset, limit: $limit) {
      id
      title
      url
      image
      author
      feedId
      publishedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    total: feedEntryCount(query: $query)
  }
  ${T}
`,Je=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${F}
  ${w}
`,Ye=`
  query bucketsTags($type: MediaDataType!, $tagType: DataType!) {
    tags(type: $tagType) {
      ...TagFragment
    }
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      coverItems { path updatedAt thumbHash }
    }
  }
  ${w}
`,Xe=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${F}
  ${I}
`,Ze=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,Qe=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,$e=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: DocSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${oe}
`,et=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,tt=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,nt=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,rt=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      feedId
      count
    }
  }
`,it=`
  query {
    total: contactCount(query: "")
  }
`,at=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:INCOMING")
    outgoing: callCount(query: "type:OUTGOING")
    missed: callCount(query: "type:MISSED")
  }
`,ot=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,st=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${k}
`,ct=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${A}
`,lt=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,ut=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${L}
`,dt=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,ft=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,pt=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,mt=`
  query {
    screenMirrorControlEnabled
  }
`,ht=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,gt=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${R}
`,_t=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${z}
`,vt=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${B}
  ${V}
`,yt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,bt=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,xt=`
  query {
    appLogPath
  }
`,St=`
  query {
    dbPath
  }
`,Ct=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId) { index sizeBytes }
  }
`,wt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,Tt=`
  query {
    pomodoroToday {
      date
      completedCount
      currentRound
      timeLeftSec
      totalTimeSec
      isRunning
      isPaused
      state
    }
    pomodoroSettings {
      workDurationMin
      shortBreakDurationMin
      longBreakDurationMin
      pomodorosBeforeLongBreak
      showNotification
      playSoundOnComplete
    }
  }
`,Et=`
  query {
    prefs
  }
`,$=`
  query {
    dbTables {
      name
      createSql
      columns {
        name
        dataType
        notNull
        defaultValue
        primaryKey
      }
    }
  }
`,Dt=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,Ot=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,kt=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${H}
  ${U}
`,At=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,jt=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    isDiscovering
  }
`,Mt=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${N}
`,Nt=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${N}
`,Pt=`
  query exportFeeds {
    exportFeeds
  }
`,Ft=`
  query exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`,It=`
  query trashedFiles($offset: Int!, $limit: Int!) {
    trashedFiles(query: "", offset: $offset, limit: $limit) {
      name path size updatedAt isDir childCount
    }
  }
`;export{wt as $,et as A,le as At,Ae as B,P as Bt,Dt as C,It as Ct,yt as D,de as Dt,vt as E,je as Et,qe as F,O as Ft,ke as G,Nt as H,rt as I,j as It,pe as J,Oe as K,Xe as L,I as Lt,Pt as M,H as Mt,Ft as N,U as Nt,Pe as O,Y as Ot,Re as P,W as Pt,Be as Q,Ke as R,F as Rt,St as S,He as St,$ as T,tt as Tt,Ne as U,be as V,w as Vt,Ze as W,ft as X,fe as Y,Ue as Z,me as _,Ce as _t,At as a,gt as at,Ee as b,xe as bt,Qe as c,ut as ct,kt as d,Et as dt,Ie as et,Ye as f,Fe as ft,ge as g,Se as gt,ye as h,pt as ht,bt as i,We as it,$e as j,J as jt,Le as k,X as kt,Ve as l,he as lt,De as m,mt,ze as n,lt as nt,st as o,nt as ot,at as p,ht as pt,Mt as q,xt as r,Ge as rt,ct as s,dt as st,_e as t,jt as tt,Me as u,Tt as ut,_t as v,we as vt,Ot as w,Ct as wt,Te as x,ve as xt,it as y,ot as yt,Je as z,N as zt};