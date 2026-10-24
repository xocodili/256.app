# Council Member Asset Layout

Each council member has a dedicated folder:

- `static/generated/council/<handle>/avatar-default.svg`
- `static/generated/council/<handle>/artworks/`
- `static/generated/council/<handle>/notes/working-memory.json`

The seed migration stores `avatar-default.svg` as the default `user_faces.generated_artwork_path`.

To replace a default avatar, overwrite:

- `static/generated/council/<handle>/avatar-default.svg`

You can also add generated artwork files to:

- `static/generated/council/<handle>/artworks/`
