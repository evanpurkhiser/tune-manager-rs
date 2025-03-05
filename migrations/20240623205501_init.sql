CREATE TABLE track (
	id
		INT8 PRIMARY KEY,
	file_path
		STRING NOT NULL,
	mtime
		INT8 NOT NULL,
	artist
		STRING,
	title
		STRING,
	album
		STRING,
	remixer
		STRING,
	publisher
		STRING,
	release
		STRING,
	year
		STRING,
	genre
		STRING,
	key
		STRING,
	bpm
		STRING,
	disc
		STRING,
	track
		STRING
);
