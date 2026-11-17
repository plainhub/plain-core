import"./chunk-native-invoke-gIPiMner.js";import{n as e,t}from"./chunk-error-code-CcA5a4Y4.js";import"./index-Di433oyf.js";import{a as n,o as r}from"./chunk-client-id-Bv9MIE_O.js";import{W as i,d as a,n as o,ot as s,st as c,z as l}from"./chunk-http-DehGcaJH.js";import{_ as u}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{A as d,F as f,L as p,j as m,tt as h,x as g}from"./chunk-_plugin-vue_export-helper-DfHvB5Qj.js";import{C as _,i as v,l as y}from"./chunk-gql-client-Be98slQ_.js";import{t as b}from"./chunk-websocket-crypto-BD1S7v9m.js";function x(e,t){return S(e,t)}function S(e,t){let s=l(`/graphql`,t),c=new URL(s,a()),u=n(),d=i();if(!d)throw Error(`Subscription session missing`);c.searchParams.set(`cid`,u);let f=o(c.toString(),r());f.binaryType=`arraybuffer`;let p=!0,m=!1,h,g,v,x=setTimeout(()=>C(Error(`Subscription handshake timeout`)),1e4),S={send(e){try{if(!p||!m||!h)throw Error(`Subscription connection unavailable`);s!==`/graphql`&&ee(e)&&(e={...e,payload:{...e.payload,query:y(e.payload.query)}}),f.send(h.encode(new TextEncoder().encode(JSON.stringify(e))))}catch(e){C(e)}},close(){p=!1,h=void 0,clearTimeout(x),clearTimeout(v),clearInterval(g),f.close()}};function C(t){p&&(S.close(),e.dropped(t))}return f.onopen=()=>{if(p)try{h=new b(u,_(d)),f.send(h.registration)}catch(e){C(e)}},f.onmessage=t=>{if(p)try{if(!h||!(t.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(m){let n=JSON.parse(new TextDecoder().decode(h.decode(new Uint8Array(t.data))));typeof n==`object`&&n&&`type`in n&&n.type===`pong`?clearTimeout(v):e.message(n)}else{if(h.acknowledge(new Uint8Array(t.data)),f.send(h.encode(new Uint8Array)),m=!0,clearTimeout(x),!p||(e.opened(),!p))return;g=setInterval(()=>{S.send({type:`ping`}),p&&(v=setTimeout(()=>C(Error(`Subscription heartbeat timeout`)),1e4))},2e4)}}catch(e){C(e)}},f.onerror=()=>C(Error(`Subscription connection error`)),f.onclose=()=>C(Error(`Subscription connection closed`)),S}function ee(e){if(!e||typeof e!=`object`||!(`type`in e)||e.type!==`subscribe`||!(`payload`in e))return!1;let t=e.payload;return!!t&&typeof t==`object`&&`query`in t&&typeof t.query==`string`}var C=class{openConnection;constructor(e=x){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(n){if(!n||typeof n!=`object`||!(`type`in n)||!(`id`in n)||typeof n.id!=`string`)throw Error(`Invalid subscription message`);let r=n,i=this.operations.get(this.ids.get(r.id)??``);if(i)if(r.type===`next`){if(!r.payload||Array.isArray(r.payload))throw Error(`Invalid subscription result`);r.payload.extensions?.subscriptionReady&&this.settled(r.id),r.payload.errors?.length&&console.error(`GraphQL subscription errors`,i.field,r.payload.errors),r.payload.data&&Object.hasOwn(r.payload.data,i.field)&&this.deliver(i,r.payload.data[i.field])}else if(r.type===`error`){if(!Array.isArray(r.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,i.field,r.payload),r.payload.some(n=>e(n.extensions?.code)===t.ResyncRequired)){this.ids.delete(r.id),this.pending.delete(r.id),this.attaching=!0,this.send(i);return}this.ids.delete(r.id),this.settled(r.id)}else if(r.type===`complete`)this.ids.delete(r.id),this.settled(r.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!c()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},w=new C,T=`
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
`,E=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,D=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,O=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,te=`
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
`,k=`
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
`,A=`
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
  ${D}
`,j=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,M=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,N=`
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
  ${D}
`,P=`
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
  ${D}
`,F=`
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
`,I=`
  fragment ImageFragment on Image {
    id
    title
    path
    size
    bucketId
    takenAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${D}
`,L=`
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
    tags {
      ...TagSubFragment
    }
  }
  ${D}
`,ne=`
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
  ${D}
`,R=`
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
  ${D}
`,re=`
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
  ${D}
`,z=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,B=`
  fragment FeedEntryFragment on FeedEntry {
    id
    title
    url
    image
    author
    description
    content
    feedId
    rawId
    publishedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${D}
`,V=`
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
`,H=`
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
`,U=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,W=`
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
`,G=`
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
`,K=`
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
`,q=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,J=`
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

`,Y={audios:`audioCount(query: $mediaQuery)`,images:`imageCount(query: $mediaQuery)`,videos:`videoCount(query: $mediaQuery)`,docs:`docCount(query: "")`,packages:`packageCount(query: "")`,notes:`noteCount(query: "")`,feedEntries:`feedEntryCount(query: "")`,messages:`smsCount(query: "")`,calls:`callCount(query: "")`,contacts:`contactCount(query: "")`};function X(e=[]){let t=e.map(e=>Y[e]).join(`
    `);return`
  query homeStats${t.includes(`$mediaQuery`)?`($mediaQuery: String!)`:``} {
    ${t}
    mounts {
      ...MountFragment
    }
  }
  ${T}
`}function Z(e){return e instanceof s?e.status===403?t.RemoteAccessDisabled:e.message:t.NetworkError}function Q(e){if(e)return typeof e==`function`?e():e}function ie(e){let t=u(!1),n=u(),r=0;async function i(i,a=!1){if(e.enabled&&!e.enabled())return;let o=r;t.value=!0;try{let t=i??Q(e.variables),s=await v(typeof e.document==`function`?e.document():e.document,t,{fresh:a});if(o!==r)return;s.errors?.length?e.handle(s.data,s.errors[0].message):(n.value=s.data,e.handle(s.data,``))}catch(t){if(o!==r||t instanceof s&&t.canceled)return;e.handle(void 0,Z(t))}finally{o===r&&(t.value=!1)}}i();let a=!0,o=g();return o&&(f(()=>{a=!1}),m(()=>{a=!0})),typeof e.variables==`function`&&h(e.variables,async()=>{await d(),a&&i()},{deep:!0}),typeof e.document==`function`&&h(e.document,async()=>{await d(),a&&i()}),e.enabled&&h(e.enabled,async(e,t)=>{await d(),a&&e&&!t&&i()}),o&&p(w.onAttached(()=>{a&&(r++,i(void 0,!0))})),{loading:t,result:n,refetch:i}}function ae(e){let t=u(!1),n=u(),r=0,i=0,a=0,o,c=0,l=!0,d;function h(){c++,a=0,o=void 0,t.value=!1}if(g()){m(()=>{l=!0}),f(()=>{l=!1,h()});let e=w.onAttached(()=>{l&&d&&_(d.variables,{...d.options,latest:!0,force:!0})});p(()=>{h(),e()})}async function _(l,u={}){u.force&&h();let f=c;d={variables:l,options:u};let p=++r,m=u.latest?++i:void 0,g=l??Q(e.variables),_=g?{...g}:void 0,y={variables:_,requestId:p,meta:u.meta};u.latest?o=p:a++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=u.force?await v(t,_,{fresh:!0}):await v(t,_);if(f!==c||u.latest&&m!==i)return;r.errors?.length?e.handle(r.data,r.errors[0].message,y):(n.value=r.data,e.handle(r.data,``,y))}catch(t){if(f!==c||t instanceof s&&t.canceled)return;(!u.latest||m===i)&&e.handle(void 0,Z(t),y)}finally{f===c&&(u.latest?o===p&&(o=void 0):a--,t.value=a>0||o!==void 0)}}return{loading:t,result:n,fetch:_}}var oe=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${k}
`;`${k}`;var se=`
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
`,ce=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${k}
`,le=`
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
`,ue=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,de=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${J}
`,fe=`
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
`,pe=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${A}
`,me=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,he=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${j}
`,ge=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${M}
`,_e=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${N}
`,ve=`
  query {
    contactSources {
      name
      type
    }
  }
`,ye=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${P}
`,be=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: ImageSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${I}
`,xe=`
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
`,Se=`
  query($volumeId: ID) {
    fileIndexScanProgress(volumeId: $volumeId) { volumeId
      indexed
      pending
      total
      state
      error
    }
  }
`,Ce=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: VideoSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${L}
`,we=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: AudioSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${ne}
`,Te=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${F}
`,Ee=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,De=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${F}
`,Oe=`
  query {
    mounts {
      ...MountFragment
    }
  }
  ${T}
`,ke=`
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
`,Ae=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,je=`
  query {
    app {
      ...AppFragment
    }
  }
  ${te}
`,Me=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Ne=`
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
  ${O}
`,Pe=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${E}
`,Fe=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,Ie=`
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
  ${D}
`,Le=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${R}
`,Re=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${z}
`,ze=`
  query feedEntries($offset: Int!, $limit: Int!, $query: String!) {
    items: feedEntries(query: $query, offset: $offset, limit: $limit) {
      id
      title
      url
      image
      author
      feedId
      rawId
      publishedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    total: feedEntryCount(query: $query)
  }
  ${D}
`,Be=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${z}
  ${E}
`,Ve=`
  query bucketsTags($type: MediaDataType!, $tagType: DataType!) {
    tags(type: $tagType) {
      ...TagFragment
    }
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
  ${E}
`,He=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${z}
  ${B}
`,Ue=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,We=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Ge=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: DocSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${re}
`,Ke=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,qe=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Je=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,Ye=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      feedId
      count
    }
  }
`,Xe=`
  query {
    total: contactCount(query: "")
  }
`,Ze=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,Qe=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,$e=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${j}
`,et=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${M}
`,tt=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,nt=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${V}
`,rt=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,it=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,at=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,ot=`
  query {
    screenMirrorControlEnabled
  }
`,st=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,ct=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${H}
`,lt=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${U}
`,ut=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${W}
  ${G}
`,dt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,ft=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,pt=`
  query {
    appLogPath
  }
`,mt=`
  query {
    dbPath
  }
`,ht=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId) { index sizeBytes }
  }
`,gt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,_t=`
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
`,vt=`
  query {
    prefs
  }
`,yt=`
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
`,$=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,bt=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,xt=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${K}
  ${q}
`,St=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,Ct=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    isDiscovering
  }
`,wt=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${F}
`,Tt=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${F}
`,Et=`
  query exportFeeds {
    exportFeeds
  }
`,Dt=`
  query exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`;export{gt as $,Ke as A,J as At,Se as B,w as Bt,$ as C,ht as Ct,dt as D,O as Dt,ut as E,X as Et,ze as F,z as Ft,xe as G,Tt as H,Ye as I,F as It,ae as J,be as K,He as L,R as Lt,Et as M,U as Mt,Dt as N,N as Nt,Ee as O,K as Ot,Ae as P,B as Pt,Me as Q,Re as R,E as Rt,mt as S,Pe as St,yt as T,Ce as Tt,Te as U,fe as V,x as Vt,Ue as W,it as X,ie as Y,Fe as Z,oe as _,he as _t,St as a,ct as at,ve as b,pe as bt,We as c,nt as ct,xt as d,vt as dt,Oe as et,Ve as f,De as ft,ce as g,me as gt,de as h,at as ht,ft as i,Ie as it,Ge as j,k as jt,ke as k,q as kt,Ne as l,se as lt,ye as m,ot as mt,je as n,tt as nt,$e as o,Je as ot,Ze as p,st as pt,wt as q,pt as r,Le as rt,et as s,rt as st,le as t,Ct as tt,we as u,_t as ut,lt as v,ge as vt,bt as w,qe as wt,_e as x,ue as xt,Xe as y,Qe as yt,Be as z,C as zt};