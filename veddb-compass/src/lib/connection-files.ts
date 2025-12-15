import { save, open } from '@tauri-apps/api/dialog';
import { writeTextFile, readTextFile } from '@tauri-apps/api/fs';
import { Connection } from '@/store';

export interface VedDBConnectionFile {
  version: string;
  connection: {
    name: string;
    host: string;
    port: number;
    username?: string;
    tls: boolean;
  };
}

export async function exportConnection(connection: Connection): Promise<void> {
  try {
    const filePath = await save({
      filters: [{
        name: 'VedDB Connection',
        extensions: ['veddb']
      }],
      defaultPath: `${connection.name.replace(/[^a-zA-Z0-9]/g, '_')}.veddb`
    });

    if (filePath) {
      const connectionFile: VedDBConnectionFile = {
        version: '1.0',
        connection: {
          name: connection.name,
          host: connection.host,
          port: connection.port,
          username: connection.username,
          tls: connection.tls,
          // Note: We don't export passwords for security reasons
        }
      };

      await writeTextFile(filePath, JSON.stringify(connectionFile, null, 2));
    }
  } catch (error) {
    console.error('Failed to export connection:', error);
    throw new Error(`Failed to export connection: ${error}`);
  }
}

export async function importConnection(): Promise<Omit<Connection, 'id' | 'isConnected'> | null> {
  try {
    const selected = await open({
      filters: [{
        name: 'VedDB Connection',
        extensions: ['veddb']
      }],
      multiple: false
    });

    if (selected && typeof selected === 'string') {
      const content = await readTextFile(selected);
      const connectionFile: VedDBConnectionFile = JSON.parse(content);

      // Validate the file format
      if (!connectionFile.version || !connectionFile.connection) {
        throw new Error('Invalid connection file format');
      }

      const { connection } = connectionFile;
      
      // Validate required fields
      if (!connection.name || !connection.host || !connection.port) {
        throw new Error('Missing required connection fields');
      }

      return {
        name: connection.name,
        host: connection.host,
        port: connection.port,
        username: connection.username,
        password: undefined, // Password not stored in file
        tls: connection.tls || false,
      };
    }

    return null;
  } catch (error) {
    console.error('Failed to import connection:', error);
    throw new Error(`Failed to import connection: ${error}`);
  }
}