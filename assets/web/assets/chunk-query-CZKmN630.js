import"./chunk-native-invoke-gIPiMner.js";import"./index-BRjW5xKv.js";import{a as e,o as t}from"./chunk-client-id-CR6HxfRu.js";import{p as n}from"./chunk-login-peers-DyqWFQb7.js";import{_ as r}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{A as i,F as a,L as o,j as s,tt as c,x as l}from"./chunk-_plugin-vue_export-helper-DfHvB5Qj.js";import{W as u,d,n as f,z as p}from"./chunk-http-6IKeMxes.js";import{a as m,t as h,u as g,w as _}from"./chunk-gql-client-BFXjyTJr.js";import{t as v}from"./chunk-websocket-crypto-DqLvIB3S.js";function y(e,t){return b(e,t)}function b(n,r){let i=p(`/graphql`,r),a=new URL(i,d()),o=e(),s=u();if(!s)throw Error(`Subscription session missing`);a.searchParams.set(`cid`,o);let c=f(a.toString(),t());c.binaryType=`arraybuffer`;let l=!0,m=!1,h,y,b,S=setTimeout(()=>w(Error(`Subscription handshake timeout`)),1e4),C={send(e){try{if(!l||!m||!h)throw Error(`Subscription connection unavailable`);i!==`/graphql`&&x(e)&&(e={...e,payload:{...e.payload,query:g(e.payload.query)}}),c.send(h.encode(new TextEncoder().encode(JSON.stringify(e))))}catch(e){w(e)}},close(){l=!1,h=void 0,clearTimeout(S),clearTimeout(b),clearInterval(y),c.close()}};function w(e){l&&(C.close(),n.dropped(e))}return c.onopen=()=>{if(l)try{h=new v(o,_(s)),c.send(h.registration)}catch(e){w(e)}},c.onmessage=e=>{if(l)try{if(!h||!(e.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(m){let t=JSON.parse(new TextDecoder().decode(h.decode(new Uint8Array(e.data))));typeof t==`object`&&t&&`type`in t&&t.type===`pong`?clearTimeout(b):n.message(t)}else{if(h.acknowledge(new Uint8Array(e.data)),c.send(h.encode(new Uint8Array)),m=!0,clearTimeout(S),!l||(n.opened(),!l))return;y=setInterval(()=>{C.send({type:`ping`}),l&&(b=setTimeout(()=>w(Error(`Subscription heartbeat timeout`)),1e4))},2e4)}}catch(e){w(e)}},c.onerror=()=>w(Error(`Subscription connection error`)),c.onclose=()=>w(Error(`Subscription connection closed`)),C}function x(e){if(!e||typeof e!=`object`||!(`type`in e)||e.type!==`subscribe`||!(`payload`in e))return!1;let t=e.payload;return!!t&&typeof t==`object`&&`query`in t&&typeof t.query==`string`}var S=class{openConnection;constructor(e=y){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&this.settled(t.id),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>e.message===`resync_required`)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!n()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},C=new S,w=`
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
`,T=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,E=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,D=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,O=`
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
`,ee=`
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
  ${E}
`,A=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,j=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,M=`
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
  ${E}
`,te=`
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
  ${E}
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
`,P=`
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
  ${E}
`,F=`
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
  ${E}
`,I=`
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
  ${E}
`,L=`
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
  ${E}
`,R=`
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
  ${E}
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
  ${E}
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
  ${w}
`}function Z(e){return e instanceof h?e.status===403?`remote_access_disabled`:e.message:`network_error`}function Q(e){if(e)return typeof e==`function`?e():e}function ne(e){let t=r(!1),n=r(),u=0;async function d(r,i=!1){if(e.enabled&&!e.enabled())return;let a=u;t.value=!0;try{let t=r??Q(e.variables),o=await m(typeof e.document==`function`?e.document():e.document,t,{fresh:i});if(a!==u)return;o.errors?.length?e.handle(o.data,o.errors[0].message):(n.value=o.data,e.handle(o.data,``))}catch(t){if(a!==u||t instanceof h&&t.canceled)return;e.handle(void 0,Z(t))}finally{a===u&&(t.value=!1)}}d();let f=!0,p=l();return p&&(a(()=>{f=!1}),s(()=>{f=!0})),typeof e.variables==`function`&&c(e.variables,async()=>{await i(),f&&d()},{deep:!0}),typeof e.document==`function`&&c(e.document,async()=>{await i(),f&&d()}),e.enabled&&c(e.enabled,async(e,t)=>{await i(),f&&e&&!t&&d()}),p&&o(C.onAttached(()=>{f&&(u++,d(void 0,!0))})),{loading:t,result:n,refetch:d}}function re(e){let t=r(!1),n=r(),i=0,c=0,u=0,d,f=0,p=!0,g;function _(){f++,u=0,d=void 0,t.value=!1}if(l()){s(()=>{p=!0}),a(()=>{p=!1,_()});let e=C.onAttached(()=>{p&&g&&v(g.variables,{...g.options,latest:!0,force:!0})});o(()=>{_(),e()})}async function v(r,a={}){a.force&&_();let o=f;g={variables:r,options:a};let s=++i,l=a.latest?++c:void 0,p=r??Q(e.variables),v=p?{...p}:void 0,y={variables:v,requestId:s,meta:a.meta};a.latest?d=s:u++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=a.force?await m(t,v,{fresh:!0}):await m(t,v);if(o!==f||a.latest&&l!==c)return;r.errors?.length?e.handle(r.data,r.errors[0].message,y):(n.value=r.data,e.handle(r.data,``,y))}catch(t){if(o!==f||t instanceof h&&t.canceled)return;(!a.latest||l===c)&&e.handle(void 0,Z(t),y)}finally{o===f&&(a.latest?d===s&&(d=void 0):u--,t.value=u>0||d!==void 0)}}return{loading:t,result:n,fetch:v}}var ie=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${k}
`;`${k}`;var ae=`
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
`,oe=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${k}
`,se=`
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
`,ce=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,le=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${J}
`,ue=`
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
`,de=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${ee}
`,fe=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,pe=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${A}
`,me=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${j}
`,he=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${M}
`,ge=`
  query {
    contactSources {
      name
      type
    }
  }
`,_e=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${te}
`,ve=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: ImageSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${P}
`,ye=`
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
`,be=`
  query($volumeId: ID) {
    fileIndexScanProgress(volumeId: $volumeId) { volumeId
      indexed
      pending
      total
      state
      error
    }
  }
`,xe=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: VideoSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${F}
`,Se=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: AudioSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${I}
`,Ce=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${N}
`,we=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,Te=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${N}
`,Ee=`
  query {
    mounts {
      ...MountFragment
    }
  }
  ${w}
`,De=`
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
`,Oe=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,ke=`
  query {
    app {
      ...AppFragment
    }
  }
  ${O}
`,Ae=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,je=`
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
  ${D}
`,Me=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${T}
`,Ne=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,Pe=`
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
  ${E}
`,Fe=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${L}
`,Ie=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${z}
`,Le=`
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
  ${E}
`,Re=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${z}
  ${T}
`,ze=`
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
  ${T}
`,Be=`
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
`,Ve=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,He=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Ue=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: DocSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${R}
`,We=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,Ge=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Ke=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,qe=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      feedId
      count
    }
  }
`,Je=`
  query {
    total: contactCount(query: "")
  }
`,Ye=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,Xe=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,Ze=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${A}
`,Qe=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${j}
`,$e=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,et=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${V}
`,tt=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,nt=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,rt=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,it=`
  query {
    screenMirrorControlEnabled
  }
`,at=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,ot=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${H}
`,st=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${U}
`,ct=`
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
`,lt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,ut=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,dt=`
  query {
    appLogPath
  }
`,ft=`
  query {
    dbPath
  }
`,$=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId) { index sizeBytes }
  }
`,pt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,mt=`
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
`,ht=`
  query {
    prefs
  }
`,gt=`
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
`,_t=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,vt=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,yt=`
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
`,bt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,xt=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    isDiscovering
  }
`,St=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${N}
`,Ct=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${N}
`,wt=`
  query exportFeeds {
    exportFeeds
  }
`,Tt=`
  query exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`;export{pt as $,We as A,J as At,be as B,C as Bt,_t as C,$ as Ct,lt as D,D as Dt,ct as E,X as Et,Le as F,z as Ft,ye as G,Ct as H,qe as I,N as It,re as J,ve as K,Be as L,L as Lt,wt as M,U as Mt,Tt as N,M as Nt,we as O,K as Ot,Oe as P,B as Pt,Ae as Q,Ie as R,T as Rt,ft as S,Me as St,gt as T,xe as Tt,Ce as U,ue as V,y as Vt,Ve as W,nt as X,ne as Y,Ne as Z,ie as _,pe as _t,bt as a,ot as at,ge as b,de as bt,He as c,et as ct,yt as d,ht as dt,Ee as et,ze as f,Te as ft,oe as g,fe as gt,le as h,rt as ht,ut as i,Pe as it,Ue as j,k as jt,De as k,q as kt,je as l,ae as lt,_e as m,it as mt,ke as n,$e as nt,Ze as o,Ke as ot,Ye as p,at as pt,St as q,dt as r,Fe as rt,Qe as s,tt as st,se as t,xt as tt,Se as u,mt as ut,st as v,me as vt,vt as w,Ge as wt,he as x,ce as xt,Je as y,Xe as yt,Re as z,S as zt};