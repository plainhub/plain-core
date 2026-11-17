import"./chunk-native-invoke-gIPiMner.js";import"./index-DmxLNRck.js";import{a as e,o as t}from"./chunk-client-id-Bv9MIE_O.js";import{W as n,ct as r,d as i,lt as a,n as o,z as s}from"./chunk-http-BMocqQ2Y.js";import{_ as c}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{A as l,F as u,L as d,j as f,tt as p,x as m}from"./chunk-_plugin-vue_export-helper-DfHvB5Qj.js";import{A as h,i as g,l as _,u as v,w as y}from"./chunk-gql-client-DwkMX2-e.js";import{t as b}from"./chunk-websocket-crypto-DZAenFmy.js";var x=function(e){return e.InvalidSubscription=`INVALID_SUBSCRIPTION`,e.SubscriptionResultTooLarge=`SUBSCRIPTION_RESULT_TOO_LARGE`,e.ResyncRequired=`RESYNC_REQUIRED`,e.SlowClient=`SLOW_CLIENT`,e}({}),S=new Set(Object.values(x));function C(e){return typeof e==`string`&&S.has(e)?e:void 0}function w(e,t){return T(e,t)}function T(r,a){let c=s(`/graphql`,a),l=new URL(c,i()),u=e(),d=n();if(!d)throw Error(`Subscription session missing`);l.searchParams.set(`cid`,u);let f=o(l.toString(),t());f.binaryType=`arraybuffer`;let p=!0,m=!1,h,g,_,x=setTimeout(()=>C(Error(`Subscription handshake timeout`)),1e4),S={send(e){try{if(!p||!m||!h)throw Error(`Subscription connection unavailable`);c!==`/graphql`&&E(e)&&(e={...e,payload:{...e.payload,query:v(e.payload.query)}}),f.send(h.encode(new TextEncoder().encode(JSON.stringify(e))))}catch(e){C(e)}},close(){p=!1,h=void 0,clearTimeout(x),clearTimeout(_),clearInterval(g),f.close()}};function C(e){p&&(S.close(),r.dropped(e))}return f.onopen=()=>{if(p)try{h=new b(u,y(d)),f.send(h.registration)}catch(e){C(e)}},f.onmessage=e=>{if(p)try{if(!h||!(e.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(m){let t=JSON.parse(new TextDecoder().decode(h.decode(new Uint8Array(e.data))));typeof t==`object`&&t&&`type`in t&&t.type===`pong`?clearTimeout(_):r.message(t)}else{if(h.acknowledge(new Uint8Array(e.data)),f.send(h.encode(new Uint8Array)),m=!0,clearTimeout(x),!p||(r.opened(),!p))return;g=setInterval(()=>{S.send({type:`ping`}),p&&(_=setTimeout(()=>C(Error(`Subscription heartbeat timeout`)),1e4))},2e4)}}catch(e){C(e)}},f.onerror=()=>C(Error(`Subscription connection error`)),f.onclose=()=>C(Error(`Subscription connection closed`)),S}function E(e){if(!e||typeof e!=`object`||!(`type`in e)||e.type!==`subscribe`||!(`payload`in e))return!1;let t=e.payload;return!!t&&typeof t==`object`&&`query`in t&&typeof t.query==`string`}var D=class{openConnection;constructor(e=w){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&this.settled(t.id),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>C(e.extensions?.code)===x.ResyncRequired)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!a()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},O=new D,k=`
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
`,A=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,j=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,M=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,N=`
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
`,P=`
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
  ${j}
`,F=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,I=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,L=`
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
  ${j}
`,R=`
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
  ${j}
`,z=`
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
`,te=`
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
  ${j}
`,ne=`
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
  ${j}
`,re=`
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
  ${j}
`,B=`
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
  ${j}
`,ie=`
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
  ${j}
`,V=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,H=`
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
  ${j}
`,U=`
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
`,W=`
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
`,G=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,K=`
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
`,q=`
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
`,J=`
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
`,Y=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,X=`
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

`,ae={audios:`audioCount(query: $mediaQuery)`,images:`imageCount(query: $mediaQuery)`,videos:`videoCount(query: $mediaQuery)`,docs:`docCount(query: "")`,packages:`packageCount(query: "")`,notes:`noteCount(query: "")`,feedEntries:`feedEntryCount(query: "")`,messages:`smsCount(query: "")`,calls:`callCount(query: "")`,contacts:`contactCount(query: "")`};function oe(e=[]){let t=e.map(e=>ae[e]).join(`
    `);return`
  query homeStats${t.includes(`$mediaQuery`)?`($mediaQuery: String!)`:``} {
    ${t}
    mounts {
      ...MountFragment
    }
  }
  ${k}
`}function Z(e){return e instanceof r?e.status===403?_.RemoteAccessDisabled:e.message:h.NetworkError}function Q(e){if(e)return typeof e==`function`?e():e}function se(e){let t=c(!1),n=c(),i=0;async function a(a,o=!1){if(e.enabled&&!e.enabled())return;let s=i;t.value=!0;try{let t=a??Q(e.variables),r=await g(typeof e.document==`function`?e.document():e.document,t,{fresh:o});if(s!==i)return;r.errors?.length?e.handle(r.data,r.errors[0].message):(n.value=r.data,e.handle(r.data,``))}catch(t){if(s!==i||t instanceof r&&t.canceled)return;e.handle(void 0,Z(t))}finally{s===i&&(t.value=!1)}}a();let o=!0,s=m();return s&&(u(()=>{o=!1}),f(()=>{o=!0})),typeof e.variables==`function`&&p(e.variables,async()=>{await l(),o&&a()},{deep:!0}),typeof e.document==`function`&&p(e.document,async()=>{await l(),o&&a()}),e.enabled&&p(e.enabled,async(e,t)=>{await l(),o&&e&&!t&&a()}),s&&d(O.onAttached(()=>{o&&(i++,a(void 0,!0))})),{loading:t,result:n,refetch:a}}function ce(e){let t=c(!1),n=c(),i=0,a=0,o=0,s,l=0,p=!0,h;function _(){l++,o=0,s=void 0,t.value=!1}if(m()){f(()=>{p=!0}),u(()=>{p=!1,_()});let e=O.onAttached(()=>{p&&h&&v(h.variables,{...h.options,latest:!0,force:!0})});d(()=>{_(),e()})}async function v(c,u={}){u.force&&_();let d=l;h={variables:c,options:u};let f=++i,p=u.latest?++a:void 0,m=c??Q(e.variables),v=m?{...m}:void 0,y={variables:v,requestId:f,meta:u.meta};u.latest?s=f:o++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=u.force?await g(t,v,{fresh:!0}):await g(t,v);if(d!==l||u.latest&&p!==a)return;r.errors?.length?e.handle(r.data,r.errors[0].message,y):(n.value=r.data,e.handle(r.data,``,y))}catch(t){if(d!==l||t instanceof r&&t.canceled)return;(!u.latest||p===a)&&e.handle(void 0,Z(t),y)}finally{d===l&&(u.latest?s===f&&(s=void 0):o--,t.value=o>0||s!==void 0)}}return{loading:t,result:n,fetch:v}}var le=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${P}
`;`${P}`;var ue=`
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
`,de=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${P}
`,fe=`
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
`,pe=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,me=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${X}
`,he=`
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
`,ge=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${ee}
`,_e=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,ve=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${F}
`,ye=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${I}
`,be=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${L}
`,xe=`
  query {
    contactSources {
      name
      type
    }
  }
`,Se=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${R}
`,Ce=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: ImageSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${te}
`,we=`
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
`,Te=`
  query($volumeId: ID) {
    fileIndexScanProgress(volumeId: $volumeId) { volumeId
      indexed
      pending
      total
      state
      error
    }
  }
`,Ee=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: VideoSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${ne}
`,De=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: AudioSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${re}
`,Oe=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${z}
`,ke=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,Ae=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${z}
`,je=`
  query {
    mounts {
      ...MountFragment
    }
  }
  ${k}
`,Me=`
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
`,Ne=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,Pe=`
  query {
    app {
      ...AppFragment
    }
  }
  ${N}
`,Fe=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Ie=`
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
  ${M}
`,Le=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${A}
`,Re=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,ze=`
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
  ${j}
`,Be=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${B}
`,Ve=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${V}
`,He=`
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
  ${j}
`,Ue=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${V}
  ${A}
`,We=`
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
  ${A}
`,Ge=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${V}
  ${H}
`,Ke=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,qe=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Je=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: DocSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${ie}
`,Ye=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,Xe=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Ze=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,Qe=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      feedId
      count
    }
  }
`,$e=`
  query {
    total: contactCount(query: "")
  }
`,et=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,tt=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,nt=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${F}
`,rt=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${I}
`,it=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,at=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${U}
`,ot=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,st=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,ct=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,lt=`
  query {
    screenMirrorControlEnabled
  }
`,ut=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,dt=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${W}
`,ft=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${G}
`,pt=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${K}
  ${q}
`,mt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,ht=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,gt=`
  query {
    appLogPath
  }
`,_t=`
  query {
    dbPath
  }
`,vt=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId) { index sizeBytes }
  }
`,yt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,$=`
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
`,bt=`
  query {
    prefs
  }
`,xt=`
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
`,St=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,Ct=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,wt=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${J}
  ${Y}
`,Tt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,Et=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    isDiscovering
  }
`,Dt=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${z}
`,Ot=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${z}
`,kt=`
  query exportFeeds {
    exportFeeds
  }
`,At=`
  query exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`;export{yt as $,Ye as A,X as At,Te as B,O as Bt,St as C,vt as Ct,mt as D,M as Dt,pt as E,oe as Et,He as F,V as Ft,we as G,Ot as H,Qe as I,z as It,ce as J,Ce as K,Ge as L,B as Lt,kt as M,G as Mt,At as N,L as Nt,ke as O,J as Ot,Ne as P,H as Pt,Fe as Q,Ve as R,A as Rt,_t as S,Le as St,xt as T,Ee as Tt,Oe as U,he as V,w as Vt,Ke as W,st as X,se as Y,Re as Z,le as _,ve as _t,Tt as a,dt as at,xe as b,ge as bt,qe as c,at as ct,wt as d,bt as dt,je as et,We as f,Ae as ft,de as g,_e as gt,me as h,ct as ht,ht as i,ze as it,Je as j,P as jt,Me as k,Y as kt,Ie as l,ue as lt,Se as m,lt as mt,Pe as n,it as nt,nt as o,Ze as ot,et as p,ut as pt,Dt as q,gt as r,Be as rt,rt as s,ot as st,fe as t,Et as tt,De as u,$ as ut,ft as v,ye as vt,Ct as w,Xe as wt,be as x,pe as xt,$e as y,tt as yt,Ue as z,D as zt};