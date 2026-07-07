# fhtml class shorthand — legend

An **optional** contraction of Tailwind class tokens: a short code stands
in for a long class (`ti4` → `text-indigo-400`). It is a *superset* of
plain fhtml — **any class you don't have a code for, just write in full**;
both compile identically. Use codes where you know them to save tokens;
never guess a code.

**Turn it on:** the file's first line must be exactly `#!shorthand`.

## Colors — `{property}{color}{shade}`, no separators

Concatenate a property code, a color code, and a shade digit:
`bg-indigo-400` → `b`+`i`+`4` = `bi4`. `text-slate-900` → `tsl9`.

- **property:** bd=border dv=divide fm=from pl=placeholder vi=via to=to t=text b=bg r=ring o=outline f=fill s=stroke
- **color:** sl=slate sk=sky st=stone gy=gray gn=green zn=zinc ne=neutral rd=red ro=rose am=amber em=emerald pu=purple pk=pink o=orange y=yellow l=lime t=teal c=cyan b=blue i=indigo v=violet f=fuchsia
- **shade:** 100–900 → one digit (`4`=400, `9`=900); `50`/`950` written in full (`ti50`=text-indigo-50). bk=black w=white carry no shade (`tw`=text-white).

## Spacing / sizing — `{property}{value}`, drop the hyphens

`px-4` → `px4`, `gap-x-6` → `gx6`, `-mt-4` → `-mt4` (keep the leading `-`).
- **property:** spx=space-x spy=space-y px=px py=py pt=pt pr=pr pb=pb pl=pl mx=mx my=my mt=mt mr=mr mb=mb ml=ml gx=gap-x gy=gap-y sz=size p=p m=m g=gap w=w h=h
- **value** (Tailwind scale only): 0 0.5 1 1.5 2 2.5 3 3.5 4 5 6 7 8 9 10 11 12 14 16 20 24 28 32 36 40 44 48 52 56 60 64 72 80 96 px

## Common utilities — exact codes

`fx`=flex `ig`=inline-grid `gr`=grid `blk`=block `ib`=inline-block `hd`=hidden `rel`=relative `abs`=absolute `ic`=items-center `is`=items-start `ie`=items-end `jc`=justify-center `jb`=justify-between `je`=justify-end `js`=justify-start `fc`=flex-col `fwr`=flex-wrap `f1`=flex-1 `wf`=w-full `hf`=h-full `ws`=w-screen `hs`=h-screen `mxa`=mx-auto `rf`=rounded-full `rd`=rounded `rsm`=rounded-sm `rmd`=rounded-md `rl`=rounded-lg `rx`=rounded-xl `r2x`=rounded-2xl `sh`=shadow `shs`=shadow-sm `shm`=shadow-md `shl`=shadow-lg `bo`=border `txs`=text-xs `ts`=text-sm `tb`=text-base `tl`=text-lg `txl`=text-xl `t2x`=text-2xl `fmd`=font-medium `fsb`=font-semibold `fb`=font-bold `tc`=text-center `tr`=text-right `un`=underline `tra`=truncate `ins`=inset-0 `po`=pointer-events-none `cp`=cursor-pointer `of`=overflow-hidden `sr`=sr-only

## Variants (`hover:`, `dark:`, `sm:`, stacked `dark:hover:`)

Keep the `variant:` prefix **verbatim** and encode only the base after the
last colon: `hover:bg-blue-500` → `hover:bb5`, `dark:hover:bg-slate-800` →
`dark:hover:bsl8`. Never abbreviate the variant word itself.

## Escape

If a literal class would collide with a code, prefix it with `=`
(`=ic` compiles to the literal class `ic`, not `items-center`). Rare.
