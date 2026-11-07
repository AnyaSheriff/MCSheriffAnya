package ru.sheriffanya.mcsa;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.DocumentsProvider;
import java.io.*;

/** Только в тестовом APK: настоящий SAF-провайдер с отдельным UID и файлами. */
public final class BackupDocumentsProvider extends DocumentsProvider {
    static final String AUTHORITY="ru.sheriffanya.mcsa.test.backups",TITLE="MCSA backup test";
    private File root;
    private static final class SafeRow {
        private final MatrixCursor cursor;private final MatrixCursor.RowBuilder row;
        SafeRow(MatrixCursor cursor) { this.cursor=cursor;this.row=cursor.newRow(); }
        SafeRow add(String column,Object value) { if(cursor.getColumnIndex(column)>=0) row.add(column,value);return this; }
    }
    @Override public boolean onCreate() { root=new File(getContext().getFilesDir(),"documents");return root.isDirectory()||root.mkdirs(); }
    @Override public Cursor queryRoots(String[] projection) {
        String[] columns=projection!=null?projection:new String[]{DocumentsContract.Root.COLUMN_ROOT_ID,DocumentsContract.Root.COLUMN_DOCUMENT_ID,DocumentsContract.Root.COLUMN_TITLE,DocumentsContract.Root.COLUMN_FLAGS,DocumentsContract.Root.COLUMN_MIME_TYPES,DocumentsContract.Root.COLUMN_ICON,DocumentsContract.Root.COLUMN_AVAILABLE_BYTES};
        MatrixCursor cursor=new MatrixCursor(columns);SafeRow row=new SafeRow(cursor);
        row.add(DocumentsContract.Root.COLUMN_ROOT_ID,"test").add(DocumentsContract.Root.COLUMN_DOCUMENT_ID,"root").add(DocumentsContract.Root.COLUMN_TITLE,TITLE)
            .add(DocumentsContract.Root.COLUMN_FLAGS,DocumentsContract.Root.FLAG_SUPPORTS_CREATE|DocumentsContract.Root.FLAG_LOCAL_ONLY)
            .add(DocumentsContract.Root.COLUMN_MIME_TYPES,"application/zip").add(DocumentsContract.Root.COLUMN_ICON,android.R.drawable.ic_menu_save).add(DocumentsContract.Root.COLUMN_AVAILABLE_BYTES,new android.os.StatFs(root.getPath()).getAvailableBytes());return cursor;
    }
    private File file(String id) throws FileNotFoundException {
        if(id.equals("root")) return root;
        if(!id.matches("[A-Za-z0-9_.-]+\\.zip")) throw new FileNotFoundException("Invalid test document id");return new File(root,id);
    }
    private MatrixCursor cursor(String[] projection) {
        return new MatrixCursor(projection!=null?projection:new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID,DocumentsContract.Document.COLUMN_DISPLAY_NAME,DocumentsContract.Document.COLUMN_MIME_TYPE,DocumentsContract.Document.COLUMN_SIZE,DocumentsContract.Document.COLUMN_FLAGS,DocumentsContract.Document.COLUMN_LAST_MODIFIED});
    }
    private void row(MatrixCursor cursor,File file) {
        boolean dir=file.equals(root);
        new SafeRow(cursor).add(DocumentsContract.Document.COLUMN_DOCUMENT_ID,dir?"root":file.getName()).add(DocumentsContract.Document.COLUMN_DISPLAY_NAME,dir?TITLE:file.getName())
            .add(DocumentsContract.Document.COLUMN_MIME_TYPE,dir?DocumentsContract.Document.MIME_TYPE_DIR:"application/zip").add(DocumentsContract.Document.COLUMN_SIZE,file.length())
            .add(DocumentsContract.Document.COLUMN_FLAGS,dir?DocumentsContract.Document.FLAG_DIR_SUPPORTS_CREATE:DocumentsContract.Document.FLAG_SUPPORTS_WRITE)
            .add(DocumentsContract.Document.COLUMN_LAST_MODIFIED,file.lastModified());
    }
    @Override public Cursor queryDocument(String id,String[] projection) throws FileNotFoundException { MatrixCursor cursor=cursor(projection);File file=file(id);if(!file.exists()) throw new FileNotFoundException(id);row(cursor,file);return cursor; }
    @Override public Cursor queryChildDocuments(String id,String[] projection,String sortOrder) throws FileNotFoundException {
        MatrixCursor cursor=cursor(projection);File[] files=file(id).listFiles();if(files!=null) for(File file:files) row(cursor,file);return cursor;
    }
    @Override public String createDocument(String parent,String mime,String name) throws FileNotFoundException {
        if(!parent.equals("root")||!mime.equals("application/zip")) throw new FileNotFoundException("Invalid test document");File file=file(name);
        try { if(!file.createNewFile()) throw new IOException("Already exists");return name; } catch(IOException error) { throw new FileNotFoundException(error.getMessage()); }
    }
    @Override public ParcelFileDescriptor openDocument(String id,String mode,CancellationSignal signal) throws FileNotFoundException { return ParcelFileDescriptor.open(file(id),ParcelFileDescriptor.parseMode(mode)); }
}
