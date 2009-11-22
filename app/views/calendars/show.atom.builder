xml.instruct! :xml, :version=>"1.0", :encoding=>"UTF-8"
xml.feed "xml:lang" => "en-US", "xmlns" => 'http://www.w3.org/2005/Atom' do
  xml.title @calendar.title
  xml.id "tag:protest.net,2009:pnet,cal/#{@calendar.id}"
  xml.link "rel" => "self", "type" => "application/atom+xml", "href" => url_for(:only_path => false)
  
  #xml.updated @items.first.updated_at.xmlschema unless @items.empty?
  
  @calendar.events.each do |event|
    xml.entry do
      xml.id "tag:protest.net,#{event.starts_at.strftime('%Y-%m-%d')}:event:#{event.id}"
      xml.published event.created_at.xmlschema
      xml.updated event.updated_at.xmlschema
      
      xml.title event.title

      #TODO: decide if description is HMTL or markdown and handle appropriately
      xml.summary event.description
      #xml.summary safe_format(event.description), 'type' => 'html'

      xml.link({:rel => 'alternate', :type => 'text/html', :href => calendar_event_url(event)})
      #xml.author{ xml.name item.created_by.name }
    end
  end
end
